//! SynapRPC frame codec.
//!
//! Wire format:
//! ```text
//! ┌───────────────────┬──────────────────────────┐
//! │  length: u32 (LE) │  body: MessagePack bytes  │
//! └───────────────────┴──────────────────────────┘
//!     4 bytes              length bytes
//! ```
//!
//! Both `Request` and `Response` use the same framing.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::types::{Request, Response};

/// Encode any `Serialize` value into a length-prefixed MessagePack frame.
pub fn encode_frame<T: Serialize>(msg: &T) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    let body = rmp_serde::to_vec(msg)?;
    let len = body.len() as u32;
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Decode one frame from a byte slice. Returns `(value, bytes_consumed)` or `None`
/// if the buffer does not yet contain a complete frame.
pub fn decode_frame<T: for<'de> Deserialize<'de>>(
    buf: &[u8],
) -> Result<Option<(T, usize)>, rmp_serde::decode::Error> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let total = 4 + len;
    if buf.len() < total {
        return Ok(None);
    }
    let value = rmp_serde::from_slice(&buf[4..total])?;
    Ok(Some((value, total)))
}

// ── Async helpers for use inside the server connection tasks ─────────────────

/// Read one `Request` frame from an async reader.
pub async fn read_request<R: AsyncRead + Unpin>(reader: &mut R) -> std::io::Result<Request> {
    read_frame(reader).await
}

/// Read one `Response` frame from an async reader.
pub async fn read_response<R: AsyncRead + Unpin>(reader: &mut R) -> std::io::Result<Response> {
    read_frame(reader).await
}

/// Write a `Request` frame to an async writer.
pub async fn write_request<W: AsyncWrite + Unpin>(
    writer: &mut W,
    req: &Request,
) -> std::io::Result<()> {
    write_frame(writer, req).await
}

/// Write a `Response` frame to an async writer.
pub async fn write_response<W: AsyncWrite + Unpin>(
    writer: &mut W,
    resp: &Response,
) -> std::io::Result<()> {
    write_frame(writer, resp).await
}

async fn read_frame<T: for<'de> Deserialize<'de>, R: AsyncRead + Unpin>(
    reader: &mut R,
) -> std::io::Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    rmp_serde::from_slice(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}

async fn write_frame<T: Serialize, W: AsyncWrite + Unpin>(
    writer: &mut W,
    msg: &T,
) -> std::io::Result<()> {
    let frame = encode_frame(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    writer.write_all(&frame).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::synap_rpc::types::SynapValue;

    #[test]
    fn encode_decode_roundtrip_request() {
        let req = Request {
            id: 1,
            command: "SET".into(),
            args: vec![
                SynapValue::Str("k".into()),
                SynapValue::Bytes(vec![1, 2, 3]),
            ],
        };
        let frame = encode_frame(&req).unwrap();
        // First 4 bytes are length
        let len = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        assert_eq!(len + 4, frame.len());

        let (decoded, consumed): (Request, usize) = decode_frame(&frame).unwrap().unwrap();
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded.id, req.id);
        assert_eq!(decoded.command, req.command);
    }

    #[test]
    fn decode_returns_none_on_partial_header() {
        let result: Result<Option<(Request, usize)>, _> = decode_frame(&[0, 0]);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn decode_returns_none_on_partial_body() {
        let req = Request {
            id: 99,
            command: "GET".into(),
            args: vec![],
        };
        let mut frame = encode_frame(&req).unwrap();
        frame.truncate(frame.len() - 1); // remove last byte
        let result: Result<Option<(Request, usize)>, _> = decode_frame(&frame);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn encode_all_synap_value_variants() {
        use SynapValue::*;
        let variants = vec![
            Null,
            Bool(true),
            Int(-1),
            Float(2.71),
            Bytes(vec![0xff, 0x00]),
            Str("test".into()),
            Array(vec![Int(1), Null]),
            Map(vec![(Str("a".into()), Int(1))]),
        ];
        for v in variants {
            let req = Request {
                id: 0,
                command: "CMD".into(),
                args: vec![v],
            };
            let frame = encode_frame(&req).unwrap();
            let (decoded, _): (Request, usize) = decode_frame(&frame).unwrap().unwrap();
            assert_eq!(decoded.id, 0);
        }
    }

    #[tokio::test]
    async fn async_write_read_roundtrip() {
        use tokio::io::BufReader;

        let req = Request {
            id: 7,
            command: "MSET".into(),
            args: vec![SynapValue::Str("key".into()), SynapValue::Int(42)],
        };

        let mut buf = Vec::new();
        write_request(&mut buf, &req).await.unwrap();

        let mut cursor = BufReader::new(std::io::Cursor::new(buf));
        let decoded = read_request(&mut cursor).await.unwrap();
        assert_eq!(decoded.id, 7);
        assert_eq!(decoded.command, "MSET");
    }
}
