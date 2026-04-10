//! Hand-written RESP3 parser.
//!
//! Supports all RESP3 type prefixes:
//!   `+` simple string   `-` error        `:` integer
//!   `$` bulk string     `*` array        `_` null
//!   `,` double          `#` boolean      `=` verbatim string
//!   `~` set             `%` map          `|` attribute (consumed, not exposed)
//!   `(` big number      `;` streamed string (not supported, returns Err)
//!
//! The parser works on byte slices for zero-copy operation during benchmarks.
//! The `parse_from_reader` async variant is used by the server accept loop.

use std::str;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

/// A RESP3 value.
#[derive(Debug, Clone, PartialEq)]
pub enum Resp3Value {
    SimpleString(String),
    Error(String),
    Integer(i64),
    Double(f64),
    Boolean(bool),
    BulkString(Vec<u8>),
    Null,
    Array(Vec<Resp3Value>),
    Set(Vec<Resp3Value>),
    Map(Vec<(Resp3Value, Resp3Value)>),
    /// Verbatim string: (encoding, data)
    Verbatim(String, Vec<u8>),
    BigNumber(String),
}

impl Resp3Value {
    /// Convenience: get inner bytes (BulkString or SimpleString).
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::BulkString(b) => Some(b),
            Self::SimpleString(s) => Some(s.as_bytes()),
            _ => None,
        }
    }

    /// Convenience: decode as UTF-8 string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::SimpleString(s) => Some(s.as_str()),
            Self::BulkString(b) => str::from_utf8(b).ok(),
            _ => None,
        }
    }

    /// Convenience: get integer value.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// True only for the Null variant.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

// ── Async parser (used by the server accept loop) ────────────────────────────

/// Parse one RESP3 value from an async buffered reader.
///
/// Returns `None` on clean EOF (client closed connection).
pub async fn parse_from_reader<R: AsyncBufRead + Unpin>(
    reader: &mut R,
) -> std::io::Result<Option<Resp3Value>> {
    let mut type_byte = [0u8; 1];
    match reader.read_exact(&mut type_byte).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let value = parse_type(reader, type_byte[0]).await?;
    Ok(Some(value))
}

fn resp_err(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg)
}

async fn read_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> std::io::Result<String> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    if line.ends_with("\r\n") {
        line.truncate(line.len() - 2);
    } else if line.ends_with('\n') {
        line.truncate(line.len() - 1);
    }
    Ok(line)
}

async fn read_bulk_bytes<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    len: usize,
) -> std::io::Result<Vec<u8>> {
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).await?;
    // Consume the trailing \r\n
    let mut crlf = [0u8; 2];
    reader.read_exact(&mut crlf).await?;
    Ok(data)
}

async fn parse_type<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    prefix: u8,
) -> std::io::Result<Resp3Value> {
    match prefix {
        // Simple string: +OK\r\n
        b'+' => {
            let s = read_line(reader).await?;
            Ok(Resp3Value::SimpleString(s))
        }
        // Error: -ERR message\r\n
        b'-' => {
            let s = read_line(reader).await?;
            Ok(Resp3Value::Error(s))
        }
        // Integer: :42\r\n
        b':' => {
            let s = read_line(reader).await?;
            let n = s.parse::<i64>().map_err(|_| resp_err("invalid integer"))?;
            Ok(Resp3Value::Integer(n))
        }
        // Bulk string: $6\r\nfoobar\r\n or $-1\r\n (null bulk)
        b'$' => {
            let s = read_line(reader).await?;
            let len: i64 = s.parse().map_err(|_| resp_err("invalid bulk length"))?;
            if len < 0 {
                return Ok(Resp3Value::Null);
            }
            let data = read_bulk_bytes(reader, len as usize).await?;
            Ok(Resp3Value::BulkString(data))
        }
        // Array: *3\r\n... or *-1\r\n (null array — RESP2 compat)
        b'*' => {
            let s = read_line(reader).await?;
            let count: i64 = s.parse().map_err(|_| resp_err("invalid array count"))?;
            if count < 0 {
                return Ok(Resp3Value::Null);
            }
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let mut prefix = [0u8; 1];
                reader.read_exact(&mut prefix).await?;
                items.push(Box::pin(parse_type(reader, prefix[0])).await?);
            }
            Ok(Resp3Value::Array(items))
        }
        // Null: _\r\n
        b'_' => {
            read_line(reader).await?; // consume \r\n
            Ok(Resp3Value::Null)
        }
        // Double: ,3.14\r\n or ,inf or ,-inf or ,nan
        b',' => {
            let s = read_line(reader).await?;
            let f = match s.as_str() {
                "inf" => f64::INFINITY,
                "-inf" => f64::NEG_INFINITY,
                "nan" => f64::NAN,
                other => other
                    .parse::<f64>()
                    .map_err(|_| resp_err("invalid double"))?,
            };
            Ok(Resp3Value::Double(f))
        }
        // Boolean: #t\r\n or #f\r\n
        b'#' => {
            let s = read_line(reader).await?;
            let b = match s.as_str() {
                "t" => true,
                "f" => false,
                _ => return Err(resp_err("invalid boolean")),
            };
            Ok(Resp3Value::Boolean(b))
        }
        // Verbatim string: =15\r\ntxt:Hello World\r\n
        b'=' => {
            let s = read_line(reader).await?;
            let len: usize = s.parse().map_err(|_| resp_err("invalid verbatim length"))?;
            let raw = read_bulk_bytes(reader, len).await?;
            // First 4 bytes are "enc:" prefix
            if raw.len() < 4 {
                return Err(resp_err("verbatim string too short"));
            }
            let enc = String::from_utf8_lossy(&raw[..3]).into_owned();
            let data = raw[4..].to_vec();
            Ok(Resp3Value::Verbatim(enc, data))
        }
        // Set: ~3\r\n...
        b'~' => {
            let s = read_line(reader).await?;
            let count: usize = s.parse().map_err(|_| resp_err("invalid set count"))?;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                let mut prefix = [0u8; 1];
                reader.read_exact(&mut prefix).await?;
                items.push(Box::pin(parse_type(reader, prefix[0])).await?);
            }
            Ok(Resp3Value::Set(items))
        }
        // Map: %2\r\nkey1\r\nval1\r\nkey2\r\nval2\r\n
        b'%' => {
            let s = read_line(reader).await?;
            let count: usize = s.parse().map_err(|_| resp_err("invalid map count"))?;
            let mut pairs = Vec::with_capacity(count);
            for _ in 0..count {
                let mut prefix = [0u8; 1];
                reader.read_exact(&mut prefix).await?;
                let k = Box::pin(parse_type(reader, prefix[0])).await?;
                reader.read_exact(&mut prefix).await?;
                let v = Box::pin(parse_type(reader, prefix[0])).await?;
                pairs.push((k, v));
            }
            Ok(Resp3Value::Map(pairs))
        }
        // Attribute (|): same structure as map, we parse and discard it,
        // then parse the actual value that follows.
        b'|' => {
            let s = read_line(reader).await?;
            let count: usize = s.parse().map_err(|_| resp_err("invalid attr count"))?;
            for _ in 0..count {
                let mut prefix = [0u8; 1];
                reader.read_exact(&mut prefix).await?;
                Box::pin(parse_type(reader, prefix[0])).await?;
                reader.read_exact(&mut prefix).await?;
                Box::pin(parse_type(reader, prefix[0])).await?;
            }
            // Now parse the real value
            let mut prefix = [0u8; 1];
            reader.read_exact(&mut prefix).await?;
            Box::pin(parse_type(reader, prefix[0])).await
        }
        // Big number: (3492890328409238509324850943850943825024385\r\n
        b'(' => {
            let s = read_line(reader).await?;
            Ok(Resp3Value::BigNumber(s))
        }
        other => Err(resp_err(&format!(
            "unknown RESP3 prefix: {:?}",
            other as char
        ))),
    }
}

// ── Inline command parser (for redis-cli compatibility) ──────────────────────

/// Parse a single-line inline command (e.g. `PING\r\n` or `SET key val\r\n`)
/// into a `Resp3Value::Array` of bulk strings.
pub fn parse_inline(line: &str) -> Resp3Value {
    let parts: Vec<Resp3Value> = line
        .split_ascii_whitespace()
        .map(|s| Resp3Value::BulkString(s.as_bytes().to_vec()))
        .collect();
    Resp3Value::Array(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    async fn parse(input: &[u8]) -> Resp3Value {
        let mut r = BufReader::new(std::io::Cursor::new(input));
        parse_from_reader(&mut r).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn parse_simple_string() {
        let v = parse(b"+OK\r\n").await;
        assert_eq!(v, Resp3Value::SimpleString("OK".into()));
    }

    #[tokio::test]
    async fn parse_error() {
        let v = parse(b"-ERR bad command\r\n").await;
        assert_eq!(v, Resp3Value::Error("ERR bad command".into()));
    }

    #[tokio::test]
    async fn parse_integer() {
        let v = parse(b":42\r\n").await;
        assert_eq!(v, Resp3Value::Integer(42));
        let v = parse(b":-1\r\n").await;
        assert_eq!(v, Resp3Value::Integer(-1));
    }

    #[tokio::test]
    async fn parse_bulk_string() {
        let v = parse(b"$6\r\nfoobar\r\n").await;
        assert_eq!(v.as_bytes(), Some(b"foobar".as_ref()));
    }

    #[tokio::test]
    async fn parse_null_bulk() {
        let v = parse(b"$-1\r\n").await;
        assert!(v.is_null());
    }

    #[tokio::test]
    async fn parse_null_explicit() {
        let v = parse(b"_\r\n").await;
        assert!(v.is_null());
    }

    #[tokio::test]
    async fn parse_array() {
        let v = parse(b"*2\r\n+hello\r\n:123\r\n").await;
        if let Resp3Value::Array(items) = v {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], Resp3Value::SimpleString("hello".into()));
            assert_eq!(items[1], Resp3Value::Integer(123));
        } else {
            panic!("expected array");
        }
    }

    #[tokio::test]
    async fn parse_boolean() {
        assert_eq!(parse(b"#t\r\n").await, Resp3Value::Boolean(true));
        assert_eq!(parse(b"#f\r\n").await, Resp3Value::Boolean(false));
    }

    #[tokio::test]
    async fn parse_double() {
        let v = parse(b",1.5\r\n").await;
        if let Resp3Value::Double(f) = v {
            assert!((f - 1.5_f64).abs() < 1e-9);
        } else {
            panic!("expected double");
        }
        assert_eq!(parse(b",inf\r\n").await, Resp3Value::Double(f64::INFINITY));
        assert_eq!(
            parse(b",-inf\r\n").await,
            Resp3Value::Double(f64::NEG_INFINITY)
        );
    }

    #[tokio::test]
    async fn parse_map() {
        let v = parse(b"%1\r\n+key\r\n:99\r\n").await;
        if let Resp3Value::Map(pairs) = v {
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].0, Resp3Value::SimpleString("key".into()));
            assert_eq!(pairs[0].1, Resp3Value::Integer(99));
        } else {
            panic!("expected map");
        }
    }

    #[tokio::test]
    async fn parse_set() {
        let v = parse(b"~2\r\n+a\r\n+b\r\n").await;
        if let Resp3Value::Set(items) = v {
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected set");
        }
    }

    #[tokio::test]
    async fn parse_nested_array() {
        let v = parse(b"*2\r\n*2\r\n:1\r\n:2\r\n$5\r\nhello\r\n").await;
        if let Resp3Value::Array(outer) = v {
            assert_eq!(outer.len(), 2);
            assert!(matches!(outer[0], Resp3Value::Array(_)));
            assert_eq!(outer[1].as_str(), Some("hello"));
        } else {
            panic!("expected array");
        }
    }

    #[tokio::test]
    async fn parse_split_reads() {
        // Simulate split TCP read: data arrives in two parts
        let part1 = b"*3\r\n$3\r\nSET\r\n";
        let part2 = b"$3\r\nkey\r\n$5\r\nhello\r\n";
        let full = [part1.as_ref(), part2.as_ref()].concat();
        let v = parse(&full).await;
        if let Resp3Value::Array(items) = v {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0].as_str(), Some("SET"));
            assert_eq!(items[1].as_str(), Some("key"));
            assert_eq!(items[2].as_str(), Some("hello"));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn parse_inline_command() {
        let v = parse_inline("PING");
        if let Resp3Value::Array(items) = v {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].as_str(), Some("PING"));
        } else {
            panic!("expected array");
        }

        let v = parse_inline("SET mykey myvalue");
        if let Resp3Value::Array(items) = v {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0].as_str(), Some("SET"));
        } else {
            panic!("expected array");
        }
    }

    #[tokio::test]
    async fn parse_eof_returns_none() {
        let mut r = BufReader::new(std::io::Cursor::new(b""));
        let v = parse_from_reader(&mut r).await.unwrap();
        assert!(v.is_none());
    }
}
