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
        let payload = format!(":{n}\r\n");
        self.bytes_written += payload.len();
        self.inner.write_all(payload.as_bytes()).await
    }

    pub async fn write_bulk(&mut self, data: &[u8]) -> std::io::Result<()> {
        let header = format!("${}\r\n", data.len());
        self.bytes_written += header.len() + data.len() + 2;
        self.inner.write_all(header.as_bytes()).await?;
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

async fn write_value<W: AsyncWrite + Unpin>(w: &mut W, value: &Resp3Value) -> std::io::Result<()> {
    match value {
        Resp3Value::SimpleString(s) => w.write_all(format!("+{s}\r\n").as_bytes()).await,
        Resp3Value::Error(e) => w.write_all(format!("-{e}\r\n").as_bytes()).await,
        Resp3Value::Integer(n) => w.write_all(format!(":{n}\r\n").as_bytes()).await,
        Resp3Value::Double(f) => w.write_all(format!(",{f}\r\n").as_bytes()).await,
        Resp3Value::Boolean(b) => {
            let ch = if *b { 't' } else { 'f' };
            w.write_all(format!("#{ch}\r\n").as_bytes()).await
        }
        Resp3Value::BulkString(data) => {
            w.write_all(format!("${}\r\n", data.len()).as_bytes())
                .await?;
            w.write_all(data).await?;
            w.write_all(b"\r\n").await
        }
        Resp3Value::Null => w.write_all(b"_\r\n").await,
        Resp3Value::Array(items) => {
            w.write_all(format!("*{}\r\n", items.len()).as_bytes())
                .await?;
            for item in items {
                Box::pin(write_value(w, item)).await?;
            }
            Ok(())
        }
        Resp3Value::Set(items) => {
            w.write_all(format!("~{}\r\n", items.len()).as_bytes())
                .await?;
            for item in items {
                Box::pin(write_value(w, item)).await?;
            }
            Ok(())
        }
        Resp3Value::Map(pairs) => {
            w.write_all(format!("%{}\r\n", pairs.len()).as_bytes())
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
            w.write_all(format!("={total}\r\n").as_bytes()).await?;
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
