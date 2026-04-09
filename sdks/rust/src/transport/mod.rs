//! Binary TCP transports for the Synap SDK.
//!
//! Two native protocols are supported alongside the original HTTP REST transport:
//!
//! - **SynapRPC** (default): 4-byte LE length-prefixed MessagePack frames.
//!   Same wire format as `synap-server`'s binary TCP listener.
//! - **RESP3**: Redis-compatible text protocol.
//!   Interoperable with any RESP2/RESP3 client.
//! - **Http**: Original JSON-over-HTTP REST transport (fallback for commands
//!   not yet mapped to a native protocol, e.g. pub/sub, queues, streams).

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Result, SynapError};

// ── Transport selection ───────────────────────────────────────────────────────

/// Selects the underlying binary protocol used by [`SynapClient`].
///
/// `SynapRpc` is the recommended default — it has the lowest overhead of the
/// three options (MessagePack framing vs. text or HTTP).
#[derive(Debug, Clone, Default)]
pub enum TransportMode {
    /// 4-byte LE length-prefixed MessagePack frames (fastest, default).
    #[default]
    SynapRpc,
    /// Redis-compatible RESP3 text protocol.
    Resp3,
    /// JSON over HTTP REST (original SDK transport, best compatibility).
    Http,
}

// ── Wire value type (mirrors synap_server::protocol::synap_rpc::types::SynapValue) ──

/// MessagePack-serialised variant type used on the SynapRPC wire.
///
/// Must be kept in sync with the server's `SynapValue` enum so that
/// `rmp_serde`'s externally-tagged encoding produces compatible bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum WireValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Bytes(Vec<u8>),
    Str(String),
    Array(Vec<WireValue>),
    Map(Vec<(WireValue, WireValue)>),
}

impl WireValue {
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub(crate) fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub(crate) fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Str(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub(crate) fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Convert a `WireValue` to a `serde_json::Value`.
    pub(crate) fn to_json(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(b) => json!(b),
            Self::Int(i) => json!(i),
            Self::Float(f) => json!(f),
            Self::Bytes(b) => {
                // Attempt UTF-8 decode; fall back to hex string.
                if let Ok(s) = std::str::from_utf8(b) {
                    json!(s)
                } else {
                    json!(
                        b.iter()
                            .map(|byte| format!("{:02x}", byte))
                            .collect::<String>()
                    )
                }
            }
            Self::Str(s) => json!(s),
            Self::Array(arr) => Value::Array(arr.iter().map(WireValue::to_json).collect()),
            Self::Map(pairs) => {
                let obj: serde_json::Map<String, Value> = pairs
                    .iter()
                    .filter_map(|(k, v)| k.as_str().map(|s| (s.to_string(), v.to_json())))
                    .collect();
                Value::Object(obj)
            }
        }
    }
}

// ── SynapRPC wire frames ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct RpcRequest {
    id: u32,
    command: String,
    args: Vec<WireValue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RpcResponse {
    id: u32,
    result: std::result::Result<WireValue, String>,
}

// ── SynapRPC transport ────────────────────────────────────────────────────────

/// Single persistent TCP connection to the SynapRPC listener.
///
/// Reconnects automatically on the first error.
pub(crate) struct SynapRpcTransport {
    addr: String,
    conn: Mutex<Option<TcpStream>>,
    next_id: AtomicU32,
    timeout: Duration,
}

impl SynapRpcTransport {
    pub(crate) fn new(host: &str, port: u16, timeout: Duration) -> Self {
        Self {
            addr: format!("{}:{}", host, port),
            conn: Mutex::new(None),
            next_id: AtomicU32::new(1),
            timeout,
        }
    }

    async fn do_connect(&self) -> Result<TcpStream> {
        tokio::time::timeout(self.timeout, TcpStream::connect(&self.addr))
            .await
            .map_err(|_| SynapError::Timeout)?
            .map_err(|e| SynapError::Other(format!("SynapRPC connect {}: {}", self.addr, e)))
    }

    /// Send `cmd ARGS…` and return the response value.
    pub(crate) async fn execute(&self, cmd: &str, args: Vec<WireValue>) -> Result<WireValue> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = RpcRequest {
            id,
            command: cmd.to_ascii_uppercase(),
            args,
        };
        let body = rmp_serde::to_vec(&req)
            .map_err(|e| SynapError::Other(format!("SynapRPC encode: {}", e)))?;
        let len_prefix = (body.len() as u32).to_le_bytes();

        let mut guard = self.conn.lock().await;

        for attempt in 0..2u8 {
            if guard.is_none() || attempt == 1 {
                *guard = Some(self.do_connect().await?);
            }
            let stream = guard.as_mut().expect("just set");

            // Write 4-byte length prefix + msgpack body.
            if stream.write_all(&len_prefix).await.is_err()
                || stream.write_all(&body).await.is_err()
            {
                *guard = None;
                if attempt == 0 {
                    continue;
                }
                return Err(SynapError::Other("SynapRPC write failed".into()));
            }

            // Read response frame.
            let mut len_buf = [0u8; 4];
            if stream.read_exact(&mut len_buf).await.is_err() {
                *guard = None;
                if attempt == 0 {
                    continue;
                }
                return Err(SynapError::Other("SynapRPC read failed".into()));
            }
            let resp_len = u32::from_le_bytes(len_buf) as usize;
            if resp_len > 64 * 1024 * 1024 {
                *guard = None;
                return Err(SynapError::Other("SynapRPC frame too large".into()));
            }
            let mut resp_body = vec![0u8; resp_len];
            if stream.read_exact(&mut resp_body).await.is_err() {
                *guard = None;
                if attempt == 0 {
                    continue;
                }
                return Err(SynapError::Other("SynapRPC read body failed".into()));
            }

            let resp: RpcResponse = rmp_serde::from_slice(&resp_body)
                .map_err(|e| SynapError::Other(format!("SynapRPC decode: {}", e)))?;

            return resp.result.map_err(SynapError::ServerError);
        }

        Err(SynapError::Other(
            "SynapRPC: exhausted reconnect attempts".into(),
        ))
    }

    /// Open a dedicated TCP connection, send SUBSCRIBE, and relay server-push
    /// frames (`id == u32::MAX`) to the returned channel.
    ///
    /// Returns `(subscriber_id, push_receiver)`.  The receiver yields each
    /// push frame as a `serde_json::Value` with fields `topic`, `payload`,
    /// `id`, and `timestamp`.  The background reader task exits when the TCP
    /// connection closes or the receiver is dropped.
    pub(crate) async fn subscribe_push(
        &self,
        topics: Vec<String>,
    ) -> Result<(String, tokio::sync::mpsc::UnboundedReceiver<Value>)> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = RpcRequest {
            id,
            command: "SUBSCRIBE".to_owned(),
            args: topics.iter().map(|t| WireValue::Str(t.clone())).collect(),
        };
        let body = rmp_serde::to_vec(&req)
            .map_err(|e| SynapError::Other(format!("SynapRPC subscribe encode: {}", e)))?;
        let len_prefix = (body.len() as u32).to_le_bytes();

        // Dedicated connection — not the shared `conn`.
        let mut stream = tokio::time::timeout(self.timeout, TcpStream::connect(&self.addr))
            .await
            .map_err(|_| SynapError::Timeout)?
            .map_err(|e| SynapError::Other(format!("SynapRPC subscribe connect: {}", e)))?;

        // Send SUBSCRIBE frame.
        stream
            .write_all(&len_prefix)
            .await
            .map_err(|_| SynapError::Other("SynapRPC subscribe write len failed".into()))?;
        stream
            .write_all(&body)
            .await
            .map_err(|_| SynapError::Other("SynapRPC subscribe write body failed".into()))?;

        // Read the initial response (subscriber_id etc.).
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|_| SynapError::Other("SynapRPC subscribe read len failed".into()))?;
        let resp_len = u32::from_le_bytes(len_buf) as usize;
        if resp_len > 64 * 1024 * 1024 {
            return Err(SynapError::Other(
                "SynapRPC subscribe: initial response frame too large".into(),
            ));
        }
        let mut resp_body = vec![0u8; resp_len];
        stream
            .read_exact(&mut resp_body)
            .await
            .map_err(|_| SynapError::Other("SynapRPC subscribe read body failed".into()))?;

        let resp: RpcResponse = rmp_serde::from_slice(&resp_body)
            .map_err(|e| SynapError::Other(format!("SynapRPC subscribe decode: {}", e)))?;
        let result = resp.result.map_err(SynapError::ServerError)?;

        // Extract subscriber_id from the Map response.
        let subscriber_id = match &result {
            WireValue::Map(pairs) => pairs
                .iter()
                .find_map(|(k, v)| {
                    if k.as_str() == Some("subscriber_id") {
                        v.as_str().map(str::to_owned)
                    } else {
                        None
                    }
                })
                .unwrap_or_default(),
            _ => String::new(),
        };

        // Spawn a background task that relays push frames (id == u32::MAX).
        let (push_tx, push_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
        tokio::spawn(async move {
            loop {
                let mut len_buf = [0u8; 4];
                if stream.read_exact(&mut len_buf).await.is_err() {
                    break;
                }
                let frame_len = u32::from_le_bytes(len_buf) as usize;
                if frame_len > 64 * 1024 * 1024 {
                    break;
                }
                let mut frame_body = vec![0u8; frame_len];
                if stream.read_exact(&mut frame_body).await.is_err() {
                    break;
                }
                let resp: RpcResponse = match rmp_serde::from_slice(&frame_body) {
                    Ok(r) => r,
                    Err(_) => break,
                };
                // Only forward push frames (server sentinel id).
                if resp.id == u32::MAX {
                    if let Ok(wire_val) = resp.result {
                        if push_tx.send(wire_val.to_json()).is_err() {
                            break; // receiver dropped
                        }
                    }
                }
                // Non-push frames on a dedicated subscription connection are
                // unexpected; skip silently.
            }
        });

        Ok((subscriber_id, push_rx))
    }
}

// ── RESP3 transport ───────────────────────────────────────────────────────────

struct Resp3Conn {
    writer: OwnedWriteHalf,
    reader: BufReader<OwnedReadHalf>,
}

/// Single persistent TCP connection to a RESP3 (Redis-compatible) listener.
pub(crate) struct Resp3Transport {
    addr: String,
    conn: Mutex<Option<Resp3Conn>>,
    timeout: Duration,
}

impl Resp3Transport {
    pub(crate) fn new(host: &str, port: u16, timeout: Duration) -> Self {
        Self {
            addr: format!("{}:{}", host, port),
            conn: Mutex::new(None),
            timeout,
        }
    }

    async fn do_connect(&self) -> Result<Resp3Conn> {
        let stream = tokio::time::timeout(self.timeout, TcpStream::connect(&self.addr))
            .await
            .map_err(|_| SynapError::Timeout)?
            .map_err(|e| SynapError::Other(format!("RESP3 connect {}: {}", self.addr, e)))?;
        let (r, w) = stream.into_split();
        Ok(Resp3Conn {
            writer: w,
            reader: BufReader::new(r),
        })
    }

    /// Send `cmd ARGS…` as a RESP2 inline array and parse the RESP3 response.
    pub(crate) async fn execute(&self, cmd: &str, args: Vec<WireValue>) -> Result<WireValue> {
        // Build RESP2 multibulk frame: *N\r\n$len\r\nword\r\n…
        let mut frame: Vec<u8> = Vec::with_capacity(64);
        let argc = args.len() + 1;
        frame.extend_from_slice(format!("*{}\r\n", argc).as_bytes());

        let cmd_upper = cmd.to_ascii_uppercase();
        frame.extend_from_slice(format!("${}\r\n", cmd_upper.len()).as_bytes());
        frame.extend_from_slice(cmd_upper.as_bytes());
        frame.extend_from_slice(b"\r\n");

        for arg in &args {
            let bytes = wire_value_to_resp_bytes(arg);
            frame.extend_from_slice(format!("${}\r\n", bytes.len()).as_bytes());
            frame.extend_from_slice(&bytes);
            frame.extend_from_slice(b"\r\n");
        }

        let mut guard = self.conn.lock().await;

        for attempt in 0..2u8 {
            if guard.is_none() || attempt == 1 {
                *guard = Some(self.do_connect().await?);
            }
            let conn = guard.as_mut().expect("just set");

            if conn.writer.write_all(&frame).await.is_err() {
                *guard = None;
                if attempt == 0 {
                    continue;
                }
                return Err(SynapError::Other("RESP3 write failed".into()));
            }

            match parse_resp3(&mut conn.reader).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    *guard = None;
                    if attempt == 0 {
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Err(SynapError::Other(
            "RESP3: exhausted reconnect attempts".into(),
        ))
    }
}

/// Render a `WireValue` as raw bytes for inclusion in a RESP2 bulk string.
fn wire_value_to_resp_bytes(v: &WireValue) -> Vec<u8> {
    match v {
        WireValue::Null => b"".to_vec(),
        WireValue::Bool(b) => {
            if *b {
                b"1".to_vec()
            } else {
                b"0".to_vec()
            }
        }
        WireValue::Int(i) => i.to_string().into_bytes(),
        WireValue::Float(f) => f.to_string().into_bytes(),
        WireValue::Bytes(b) => b.clone(),
        WireValue::Str(s) => s.as_bytes().to_vec(),
        WireValue::Array(_) | WireValue::Map(_) => {
            // Arrays shouldn't appear as individual args; render as empty.
            b"".to_vec()
        }
    }
}

// ── RESP3 response parser ─────────────────────────────────────────────────────

async fn read_line(reader: &mut BufReader<OwnedReadHalf>) -> Result<String> {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .map_err(|e| SynapError::Other(format!("RESP3 read: {}", e)))?;
    if line.ends_with("\r\n") {
        line.truncate(line.len() - 2);
    } else if line.ends_with('\n') {
        line.truncate(line.len() - 1);
    }
    Ok(line)
}

async fn read_bulk(reader: &mut BufReader<OwnedReadHalf>, len: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .await
        .map_err(|e| SynapError::Other(format!("RESP3 bulk read: {}", e)))?;
    // Consume trailing \r\n.
    let mut crlf = [0u8; 2];
    reader
        .read_exact(&mut crlf)
        .await
        .map_err(|e| SynapError::Other(format!("RESP3 crlf read: {}", e)))?;
    Ok(buf)
}

async fn parse_resp3(reader: &mut BufReader<OwnedReadHalf>) -> Result<WireValue> {
    let mut prefix_buf = [0u8; 1];
    reader
        .read_exact(&mut prefix_buf)
        .await
        .map_err(|e| SynapError::Other(format!("RESP3 prefix read: {}", e)))?;
    parse_resp3_type(reader, prefix_buf[0]).await
}

#[allow(clippy::manual_async_fn)] // Box::pin avoids infinite async recursion
fn parse_resp3_type(
    reader: &mut BufReader<OwnedReadHalf>,
    prefix: u8,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<WireValue>> + Send + '_>> {
    Box::pin(async move {
        match prefix {
            // Simple string: +OK\r\n
            b'+' => {
                let s = read_line(reader).await?;
                Ok(WireValue::Str(s))
            }
            // Error: -ERR message\r\n
            b'-' => {
                let s = read_line(reader).await?;
                Err(SynapError::ServerError(s))
            }
            // Integer: :42\r\n
            b':' => {
                let s = read_line(reader).await?;
                let n = s
                    .parse::<i64>()
                    .map_err(|_| SynapError::Other("RESP3 invalid integer".into()))?;
                Ok(WireValue::Int(n))
            }
            // Bulk string: $6\r\nfoobar\r\n or $-1\r\n (null)
            b'$' => {
                let s = read_line(reader).await?;
                let len: i64 = s
                    .parse()
                    .map_err(|_| SynapError::Other("RESP3 invalid bulk len".into()))?;
                if len < 0 {
                    return Ok(WireValue::Null);
                }
                let data = read_bulk(reader, len as usize).await?;
                Ok(WireValue::Str(String::from_utf8_lossy(&data).into_owned()))
            }
            // Array: *3\r\n… or *-1\r\n (null array)
            b'*' => {
                let s = read_line(reader).await?;
                let count: i64 = s
                    .parse()
                    .map_err(|_| SynapError::Other("RESP3 invalid array count".into()))?;
                if count < 0 {
                    return Ok(WireValue::Null);
                }
                let mut items = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let mut p = [0u8; 1];
                    reader
                        .read_exact(&mut p)
                        .await
                        .map_err(|e| SynapError::Other(format!("RESP3 array item: {}", e)))?;
                    items.push(parse_resp3_type(reader, p[0]).await?);
                }
                Ok(WireValue::Array(items))
            }
            // Null: _\r\n (RESP3)
            b'_' => {
                read_line(reader).await?;
                Ok(WireValue::Null)
            }
            // Double: ,3.14\r\n
            b',' => {
                let s = read_line(reader).await?;
                let f = match s.as_str() {
                    "inf" => f64::INFINITY,
                    "-inf" => f64::NEG_INFINITY,
                    other => other
                        .parse::<f64>()
                        .map_err(|_| SynapError::Other("RESP3 invalid double".into()))?,
                };
                Ok(WireValue::Float(f))
            }
            // Boolean: #t\r\n or #f\r\n (RESP3)
            b'#' => {
                let s = read_line(reader).await?;
                let b = match s.as_str() {
                    "t" => true,
                    "f" => false,
                    _ => return Err(SynapError::Other("RESP3 invalid boolean".into())),
                };
                Ok(WireValue::Bool(b))
            }
            // Map: %2\r\nkey1\r\nval1\r\nkey2\r\nval2\r\n (RESP3)
            b'%' => {
                let s = read_line(reader).await?;
                let count: usize = s
                    .parse()
                    .map_err(|_| SynapError::Other("RESP3 invalid map count".into()))?;
                let mut pairs = Vec::with_capacity(count);
                for _ in 0..count {
                    let mut p = [0u8; 1];
                    reader
                        .read_exact(&mut p)
                        .await
                        .map_err(|e| SynapError::Other(format!("RESP3 map key prefix: {}", e)))?;
                    let k = parse_resp3_type(reader, p[0]).await?;
                    reader
                        .read_exact(&mut p)
                        .await
                        .map_err(|e| SynapError::Other(format!("RESP3 map val prefix: {}", e)))?;
                    let v = parse_resp3_type(reader, p[0]).await?;
                    pairs.push((k, v));
                }
                Ok(WireValue::Map(pairs))
            }
            // Set: ~N\r\n… (RESP3 — treat as Array)
            b'~' => {
                let s = read_line(reader).await?;
                let count: usize = s
                    .parse()
                    .map_err(|_| SynapError::Other("RESP3 invalid set count".into()))?;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    let mut p = [0u8; 1];
                    reader
                        .read_exact(&mut p)
                        .await
                        .map_err(|e| SynapError::Other(format!("RESP3 set item: {}", e)))?;
                    items.push(parse_resp3_type(reader, p[0]).await?);
                }
                Ok(WireValue::Array(items))
            }
            other => Err(SynapError::Other(format!(
                "RESP3 unknown prefix: {:?}",
                other as char
            ))),
        }
    })
}

pub(crate) mod mapping;
pub(crate) use mapping::{map_command, map_response};

#[cfg(test)]
mod tests;
