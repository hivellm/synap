//! RESP3 response serialiser.
//!
//! `Resp3Writer` writes RESP3-encoded bytes to any `AsyncWrite` sink.
//! Only the response types actually sent by the server are implemented;
//! client-to-server types (`, # =` etc.) are not needed here.

use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::parser::Resp3Value;

pub struct Resp3Writer<W> {
    inner: W,
    bytes_written: usize,
}

impl<W: AsyncWrite + Unpin> Resp3Writer<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            bytes_written: 0,
        }
    }

    /// Total bytes written since this writer was created.
    pub fn bytes_written(&self) -> usize {
        self.bytes_written
    }

    /// Flush the underlying writer.
    pub async fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush().await
    }

    /// Write any `Resp3Value` to the stream.
    pub async fn write(&mut self, value: &Resp3Value) -> std::io::Result<()> {
        let mut counter = ByteCounter::new(&mut self.inner);
        write_value(&mut counter, value).await?;
        self.bytes_written += counter.count;
        Ok(())
    }

    // ── Convenience constructors ─────────────────────────────────────────────

    pub async fn write_ok(&mut self) -> std::io::Result<()> {
        self.bytes_written += 5; // "+OK\r\n"
        self.inner.write_all(b"+OK\r\n").await
    }

    pub async fn write_pong(&mut self) -> std::io::Result<()> {
        self.bytes_written += 7; // "+PONG\r\n"
        self.inner.write_all(b"+PONG\r\n").await
    }

    pub async fn write_null(&mut self) -> std::io::Result<()> {
        self.bytes_written += 5; // "$-1\r\n"
        self.inner.write_all(b"$-1\r\n").await
    }

    pub async fn write_error(&mut self, msg: &str) -> std::io::Result<()> {
        let payload = format!("-ERR {msg}\r\n");
        self.bytes_written += payload.len();
        self.inner.write_all(payload.as_bytes()).await
    }

    pub async fn write_noauth(&mut self) -> std::io::Result<()> {
        self.bytes_written += 32; // "-NOAUTH Authentication required\r\n"
        self.inner
            .write_all(b"-NOAUTH Authentication required\r\n")
            .await
    }

    pub async fn write_integer(&mut self, n: i64) -> std::io::Result<()> {
        let mut nb = [0u8; 24];
        let line = fmt_num_line(&mut nb, b':', n);
        self.bytes_written += line.len();
        self.inner.write_all(line).await
    }

    pub async fn write_bulk(&mut self, data: &[u8]) -> std::io::Result<()> {
        let mut nb = [0u8; 24];
        let header = fmt_num_line(&mut nb, b'$', data.len() as i64);
        self.bytes_written += header.len() + data.len() + 2;
        self.inner.write_all(header).await?;
        self.inner.write_all(data).await?;
        self.inner.write_all(b"\r\n").await
    }

    pub async fn write_simple(&mut self, s: &str) -> std::io::Result<()> {
        let payload = format!("+{s}\r\n");
        self.bytes_written += payload.len();
        self.inner.write_all(payload.as_bytes()).await
    }
}

/// Wraps an `AsyncWrite` to count bytes written through it.
struct ByteCounter<'a, W> {
    inner: &'a mut W,
    count: usize,
}

impl<'a, W: AsyncWrite + Unpin> ByteCounter<'a, W> {
    fn new(inner: &'a mut W) -> Self {
        Self { inner, count: 0 }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for ByteCounter<'_, W> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let poll = std::pin::Pin::new(&mut self.inner).poll_write(cx, buf);
        if let std::task::Poll::Ready(Ok(n)) = &poll {
            self.count += n;
        }
        poll
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Render `<prefix><n>\r\n` into `buf` back-to-front and return the filled
/// slice — an itoa-style stack formatter so integer replies and bulk/aggregate
/// headers never allocate (Redis does the same with `ll2string`).
/// 24 bytes fit: prefix + sign + 20 digits (i64::MIN) + CRLF.
#[inline]
fn fmt_num_line(buf: &mut [u8; 24], prefix: u8, n: i64) -> &[u8] {
    let mut pos = buf.len() - 2;
    buf[pos] = b'\r';
    buf[pos + 1] = b'\n';
    let neg = n < 0;
    let mut v = n.unsigned_abs();
    loop {
        pos -= 1;
        buf[pos] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    if neg {
        pos -= 1;
        buf[pos] = b'-';
    }
    pos -= 1;
    buf[pos] = prefix;
    &buf[pos..]
}

async fn write_value<W: AsyncWrite + Unpin>(w: &mut W, value: &Resp3Value) -> std::io::Result<()> {
    // Stack buffer for integer replies and length headers — no per-reply alloc.
    let mut nb = [0u8; 24];
    match value {
        // SimpleString/Error write in parts: the sink is buffered (BufWriter) in
        // the server, so each write_all is a memcpy into the buffer, not a syscall.
        Resp3Value::SimpleString(s) => {
            w.write_all(b"+").await?;
            w.write_all(s.as_bytes()).await?;
            w.write_all(b"\r\n").await
        }
        Resp3Value::Error(e) => {
            w.write_all(b"-").await?;
            w.write_all(e.as_bytes()).await?;
            w.write_all(b"\r\n").await
        }
        Resp3Value::Integer(n) => w.write_all(fmt_num_line(&mut nb, b':', *n)).await,
        Resp3Value::Double(f) => w.write_all(format!(",{f}\r\n").as_bytes()).await,
        Resp3Value::Boolean(b) => {
            let ch = if *b { 't' } else { 'f' };
            w.write_all(format!("#{ch}\r\n").as_bytes()).await
        }
        Resp3Value::BulkString(data) => {
            w.write_all(fmt_num_line(&mut nb, b'$', data.len() as i64))
                .await?;
            w.write_all(data).await?;
            w.write_all(b"\r\n").await
        }
        // Serialised byte-for-byte identically to BulkString; the payload is a
        // shared buffer carried from the store with no intermediate copy.
        Resp3Value::BulkShared(data) => {
            w.write_all(fmt_num_line(&mut nb, b'$', data.len() as i64))
                .await?;
            w.write_all(data).await?;
            w.write_all(b"\r\n").await
        }
        Resp3Value::Null => w.write_all(b"_\r\n").await,
        Resp3Value::Array(items) => {
            w.write_all(fmt_num_line(&mut nb, b'*', items.len() as i64))
                .await?;
            for item in items {
                Box::pin(write_value(w, item)).await?;
            }
            Ok(())
        }
        Resp3Value::Set(items) => {
            w.write_all(fmt_num_line(&mut nb, b'~', items.len() as i64))
                .await?;
            for item in items {
                Box::pin(write_value(w, item)).await?;
            }
            Ok(())
        }
        Resp3Value::Map(pairs) => {
            w.write_all(fmt_num_line(&mut nb, b'%', pairs.len() as i64))
                .await?;
            for (k, v) in pairs {
                Box::pin(write_value(w, k)).await?;
                Box::pin(write_value(w, v)).await?;
            }
            Ok(())
        }
        Resp3Value::Verbatim(enc, data) => {
            // =<len>\r\n<enc>:<data>\r\n
            let total = 4 + data.len(); // "enc:" prefix
            w.write_all(fmt_num_line(&mut nb, b'=', total as i64))
                .await?;
            w.write_all(enc.as_bytes()).await?;
            w.write_all(b":").await?;
            w.write_all(data).await?;
            w.write_all(b"\r\n").await
        }
        Resp3Value::BigNumber(n) => w.write_all(format!("({n}\r\n").as_bytes()).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn write_to_vec(value: &Resp3Value) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut w = Resp3Writer::new(&mut buf);
        w.write(value).await.unwrap();
        buf
    }

    #[tokio::test]
    async fn write_simple_string() {
        let b = write_to_vec(&Resp3Value::SimpleString("OK".into())).await;
        assert_eq!(b, b"+OK\r\n");
    }

    #[tokio::test]
    async fn write_error() {
        let b = write_to_vec(&Resp3Value::Error("ERR bad".into())).await;
        assert_eq!(b, b"-ERR bad\r\n");
    }

    #[tokio::test]
    async fn write_integer() {
        let b = write_to_vec(&Resp3Value::Integer(-5)).await;
        assert_eq!(b, b":-5\r\n");
    }

    #[tokio::test]
    async fn write_null() {
        let b = write_to_vec(&Resp3Value::Null).await;
        assert_eq!(b, b"_\r\n");
    }

    #[tokio::test]
    async fn write_bulk_string() {
        let b = write_to_vec(&Resp3Value::BulkString(b"hello".to_vec())).await;
        assert_eq!(b, b"$5\r\nhello\r\n");
    }

    #[tokio::test]
    async fn write_bulk_shared_matches_bulk_string() {
        use std::sync::Arc;
        // The shared variant must serialise byte-for-byte like the owned one.
        let owned = write_to_vec(&Resp3Value::BulkString(b"hello".to_vec())).await;
        let shared = write_to_vec(&Resp3Value::BulkShared(Arc::from(b"hello".to_vec()))).await;
        assert_eq!(owned, shared);
        assert_eq!(shared, b"$5\r\nhello\r\n");
    }

    #[tokio::test]
    async fn write_array() {
        let v = Resp3Value::Array(vec![
            Resp3Value::SimpleString("one".into()),
            Resp3Value::Integer(2),
        ]);
        let b = write_to_vec(&v).await;
        assert_eq!(b, b"*2\r\n+one\r\n:2\r\n");
    }

    #[tokio::test]
    async fn write_map() {
        let v = Resp3Value::Map(vec![(
            Resp3Value::SimpleString("k".into()),
            Resp3Value::Integer(1),
        )]);
        let b = write_to_vec(&v).await;
        assert_eq!(b, b"%1\r\n+k\r\n:1\r\n");
    }

    #[tokio::test]
    async fn roundtrip_with_parser() {
        use super::super::parser::parse_from_reader;
        use tokio::io::BufReader;

        let values = vec![
            Resp3Value::SimpleString("hello".into()),
            Resp3Value::Integer(42),
            Resp3Value::BulkString(b"binary\x00data".to_vec()),
            Resp3Value::Null,
            Resp3Value::Boolean(true),
            Resp3Value::Array(vec![Resp3Value::Integer(1), Resp3Value::Null]),
        ];

        for v in &values {
            let mut buf = Vec::new();
            let mut w = Resp3Writer::new(&mut buf);
            w.write(v).await.unwrap();

            let mut r = BufReader::new(std::io::Cursor::new(buf));
            let parsed = parse_from_reader(&mut r).await.unwrap().unwrap();
            assert_eq!(&parsed, v);
        }
    }
}
