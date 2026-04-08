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

// ── Command mapper ────────────────────────────────────────────────────────────

/// Translate a dotted SDK command + JSON payload into a raw Redis-style command
/// plus an ordered arg list for the native protocols.
///
/// Returns `None` for commands that have no native mapping (e.g. pub/sub,
/// queues, streams); the caller should fall back to HTTP in that case.
pub(crate) fn map_command(cmd: &str, payload: &Value) -> Option<(&'static str, Vec<WireValue>)> {
    // Helper: extract a string field → WireValue::Str
    let field_str = |key: &str| -> WireValue {
        match &payload[key] {
            Value::String(s) => WireValue::Str(s.clone()),
            Value::Number(n) => WireValue::Str(n.to_string()),
            Value::Bool(b) => WireValue::Str(b.to_string()),
            _ => WireValue::Str(String::new()),
        }
    };

    // Helper: convert any JSON value → WireValue
    let to_wire = |v: &Value| -> WireValue {
        match v {
            Value::Null => WireValue::Null,
            Value::Bool(b) => WireValue::Bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    WireValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    WireValue::Float(f)
                } else {
                    WireValue::Str(n.to_string())
                }
            }
            Value::String(s) => WireValue::Str(s.clone()),
            _ => WireValue::Str(v.to_string()),
        }
    };

    Some(match cmd {
        // ── KV ────────────────────────────────────────────────────────────────
        "kv.get" => ("GET", vec![field_str("key")]),

        "kv.set" => {
            let mut args = vec![field_str("key"), to_wire(&payload["value"])];
            if let Some(ttl) = payload["ttl"].as_u64() {
                args.push(WireValue::Str("EX".into()));
                args.push(WireValue::Int(ttl as i64));
            }
            ("SET", args)
        }

        "kv.del" => ("DEL", vec![field_str("key")]),
        "kv.exists" => ("EXISTS", vec![field_str("key")]),
        "kv.incr" => ("INCR", vec![field_str("key")]),
        "kv.decr" => ("DECR", vec![field_str("key")]),

        "kv.keys" => {
            let prefix = payload["prefix"].as_str().unwrap_or("");
            let pattern = if prefix.is_empty() {
                "*".to_string()
            } else {
                format!("{}*", prefix)
            };
            ("KEYS", vec![WireValue::Str(pattern)])
        }

        "kv.expire" => (
            "EXPIRE",
            vec![
                field_str("key"),
                WireValue::Int(payload["ttl"].as_u64().unwrap_or(0) as i64),
            ],
        ),

        "kv.ttl" => ("TTL", vec![field_str("key")]),

        // ── Hash ──────────────────────────────────────────────────────────────
        "hash.set" => (
            "HSET",
            vec![field_str("key"), field_str("field"), field_str("value")],
        ),
        "hash.get" => ("HGET", vec![field_str("key"), field_str("field")]),
        "hash.getall" => ("HGETALL", vec![field_str("key")]),
        "hash.del" => ("HDEL", vec![field_str("key"), field_str("field")]),
        "hash.exists" => ("HEXISTS", vec![field_str("key"), field_str("field")]),
        "hash.keys" => ("HKEYS", vec![field_str("key")]),
        "hash.values" => ("HVALS", vec![field_str("key")]),
        "hash.len" => ("HLEN", vec![field_str("key")]),

        "hash.mset" => {
            let mut args = vec![field_str("key")];
            if let Some(obj) = payload["fields"].as_object() {
                // HashMap format: {"field": "value", ...}
                for (k, v) in obj {
                    args.push(WireValue::Str(k.clone()));
                    args.push(to_wire(v));
                }
            } else if let Some(arr) = payload["fields"].as_array() {
                // Array format: [{"field": "...", "value": "..."}, ...]
                for item in arr {
                    if let Some(f) = item["field"].as_str() {
                        args.push(WireValue::Str(f.to_string()));
                        args.push(to_wire(&item["value"]));
                    }
                }
            }
            // Redis 4+ HSET handles multi-field: HSET key f1 v1 f2 v2 ...
            ("HSET", args)
        }

        "hash.mget" => {
            let mut args = vec![field_str("key")];
            if let Some(fields) = payload["fields"].as_array() {
                for f in fields {
                    if let Some(s) = f.as_str() {
                        args.push(WireValue::Str(s.to_string()));
                    }
                }
            }
            ("HMGET", args)
        }

        "hash.incrby" => (
            "HINCRBY",
            vec![
                field_str("key"),
                field_str("field"),
                WireValue::Int(payload["increment"].as_i64().unwrap_or(0)),
            ],
        ),

        "hash.incrbyfloat" => (
            "HINCRBYFLOAT",
            vec![
                field_str("key"),
                field_str("field"),
                WireValue::Str(payload["increment"].as_f64().unwrap_or(0.0).to_string()),
            ],
        ),

        "hash.setnx" => (
            "HSETNX",
            vec![field_str("key"), field_str("field"), field_str("value")],
        ),

        // ── List ──────────────────────────────────────────────────────────────
        "list.lpush" | "list.lpushx" => {
            let cmd_name: &'static str = if cmd == "list.lpushx" {
                "LPUSHX"
            } else {
                "LPUSH"
            };
            let mut args = vec![field_str("key")];
            if let Some(vals) = payload["values"].as_array() {
                for v in vals {
                    args.push(WireValue::Str(v.as_str().unwrap_or("").to_string()));
                }
            }
            (cmd_name, args)
        }

        "list.rpush" | "list.rpushx" => {
            let cmd_name: &'static str = if cmd == "list.rpushx" {
                "RPUSHX"
            } else {
                "RPUSH"
            };
            let mut args = vec![field_str("key")];
            if let Some(vals) = payload["values"].as_array() {
                for v in vals {
                    args.push(WireValue::Str(v.as_str().unwrap_or("").to_string()));
                }
            }
            (cmd_name, args)
        }

        "list.lpop" => {
            let mut args = vec![field_str("key")];
            if let Some(c) = payload["count"].as_u64() {
                args.push(WireValue::Int(c as i64));
            }
            ("LPOP", args)
        }

        "list.rpop" => {
            let mut args = vec![field_str("key")];
            if let Some(c) = payload["count"].as_u64() {
                args.push(WireValue::Int(c as i64));
            }
            ("RPOP", args)
        }

        "list.range" => (
            "LRANGE",
            vec![
                field_str("key"),
                WireValue::Int(payload["start"].as_i64().unwrap_or(0)),
                WireValue::Int(payload["stop"].as_i64().unwrap_or(-1)),
            ],
        ),

        "list.len" => ("LLEN", vec![field_str("key")]),

        "list.index" => (
            "LINDEX",
            vec![
                field_str("key"),
                WireValue::Int(payload["index"].as_i64().unwrap_or(0)),
            ],
        ),

        "list.set" => (
            "LSET",
            vec![
                field_str("key"),
                WireValue::Int(payload["index"].as_i64().unwrap_or(0)),
                to_wire(&payload["value"]),
            ],
        ),

        "list.trim" => (
            "LTRIM",
            vec![
                field_str("key"),
                WireValue::Int(payload["start"].as_i64().unwrap_or(0)),
                WireValue::Int(payload["end"].as_i64().unwrap_or(-1)),
            ],
        ),

        "list.rem" => (
            "LREM",
            vec![
                field_str("key"),
                WireValue::Int(payload["count"].as_i64().unwrap_or(0)),
                to_wire(&payload["element"]),
            ],
        ),

        "list.insert" => {
            let before_after = if payload["before"].as_bool().unwrap_or(true) {
                "BEFORE"
            } else {
                "AFTER"
            };
            (
                "LINSERT",
                vec![
                    field_str("key"),
                    WireValue::Str(before_after.into()),
                    to_wire(&payload["pivot"]),
                    to_wire(&payload["value"]),
                ],
            )
        }

        "list.rpoplpush" => (
            "RPOPLPUSH",
            vec![field_str("key"), field_str("destination")],
        ),

        "list.pos" => ("LPOS", vec![field_str("key"), to_wire(&payload["element"])]),

        // ── Set ───────────────────────────────────────────────────────────────
        "set.add" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                for m in members {
                    args.push(WireValue::Str(m.as_str().unwrap_or("").to_string()));
                }
            }
            ("SADD", args)
        }

        "set.rem" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                for m in members {
                    args.push(WireValue::Str(m.as_str().unwrap_or("").to_string()));
                }
            }
            ("SREM", args)
        }

        "set.ismember" => ("SISMEMBER", vec![field_str("key"), field_str("member")]),

        "set.members" => ("SMEMBERS", vec![field_str("key")]),
        "set.card" => ("SCARD", vec![field_str("key")]),

        "set.pop" => (
            "SPOP",
            vec![
                field_str("key"),
                WireValue::Int(payload["count"].as_u64().unwrap_or(1) as i64),
            ],
        ),

        "set.randmember" => (
            "SRANDMEMBER",
            vec![
                field_str("key"),
                WireValue::Int(payload["count"].as_u64().unwrap_or(1) as i64),
            ],
        ),

        "set.move" => (
            "SMOVE",
            vec![
                field_str("key"),
                field_str("destination"),
                field_str("member"),
            ],
        ),

        "set.inter" | "set.union" | "set.diff" => {
            let raw: &'static str = match cmd {
                "set.inter" => "SINTER",
                "set.union" => "SUNION",
                _ => "SDIFF",
            };
            let mut args: Vec<WireValue> = vec![];
            if let Some(keys) = payload["keys"].as_array() {
                for k in keys {
                    if let Some(s) = k.as_str() {
                        args.push(WireValue::Str(s.to_string()));
                    }
                }
            }
            (raw, args)
        }

        "set.interstore" | "set.unionstore" | "set.diffstore" => {
            let raw: &'static str = match cmd {
                "set.interstore" => "SINTERSTORE",
                "set.unionstore" => "SUNIONSTORE",
                _ => "SDIFFSTORE",
            };
            let mut args = vec![field_str("destination")];
            if let Some(keys) = payload["keys"].as_array() {
                for k in keys {
                    if let Some(s) = k.as_str() {
                        args.push(WireValue::Str(s.to_string()));
                    }
                }
            }
            (raw, args)
        }

        // ── Sorted Set ────────────────────────────────────────────────────────
        "sortedset.zadd" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                // add_multiple format: [{member, score}, ...]
                for m in members {
                    args.push(WireValue::Str(
                        m["score"].as_f64().unwrap_or(0.0).to_string(),
                    ));
                    args.push(WireValue::Str(
                        m["member"].as_str().unwrap_or("").to_string(),
                    ));
                }
            } else {
                // add format: {member, score}
                args.push(WireValue::Str(
                    payload["score"].as_f64().unwrap_or(0.0).to_string(),
                ));
                args.push(to_wire(&payload["member"]));
            }
            ("ZADD", args)
        }

        "sortedset.zrem" => {
            let mut args = vec![field_str("key")];
            if let Some(members) = payload["members"].as_array() {
                for m in members {
                    args.push(WireValue::Str(m.as_str().unwrap_or("").to_string()));
                }
            } else {
                args.push(to_wire(&payload["member"]));
            }
            ("ZREM", args)
        }

        "sortedset.zscore" => (
            "ZSCORE",
            vec![field_str("key"), to_wire(&payload["member"])],
        ),

        "sortedset.zcard" => ("ZCARD", vec![field_str("key")]),

        "sortedset.zincrby" => (
            "ZINCRBY",
            vec![
                field_str("key"),
                WireValue::Str(payload["increment"].as_f64().unwrap_or(0.0).to_string()),
                to_wire(&payload["member"]),
            ],
        ),

        "sortedset.zrank" => ("ZRANK", vec![field_str("key"), to_wire(&payload["member"])]),

        "sortedset.zrevrank" => (
            "ZREVRANK",
            vec![field_str("key"), to_wire(&payload["member"])],
        ),

        "sortedset.zcount" => (
            "ZCOUNT",
            vec![
                field_str("key"),
                WireValue::Str(
                    payload["min"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["min"].as_str().unwrap_or("-inf").to_string()),
                ),
                WireValue::Str(
                    payload["max"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["max"].as_str().unwrap_or("+inf").to_string()),
                ),
            ],
        ),

        "sortedset.zrange" | "sortedset.zrevrange" => {
            let raw: &'static str = if cmd == "sortedset.zrevrange" {
                "ZREVRANGE"
            } else {
                "ZRANGE"
            };
            let mut args = vec![
                field_str("key"),
                WireValue::Int(payload["start"].as_i64().unwrap_or(0)),
                WireValue::Int(payload["stop"].as_i64().unwrap_or(-1)),
            ];
            if payload["withscores"].as_bool().unwrap_or(false) {
                args.push(WireValue::Str("WITHSCORES".into()));
            }
            (raw, args)
        }

        "sortedset.zrangebyscore" => {
            let mut args = vec![
                field_str("key"),
                WireValue::Str(
                    payload["min"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["min"].as_str().unwrap_or("-inf").to_string()),
                ),
                WireValue::Str(
                    payload["max"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["max"].as_str().unwrap_or("+inf").to_string()),
                ),
            ];
            if payload["withscores"].as_bool().unwrap_or(false) {
                args.push(WireValue::Str("WITHSCORES".into()));
            }
            ("ZRANGEBYSCORE", args)
        }

        "sortedset.zpopmin" | "sortedset.zpopmax" => {
            let raw: &'static str = if cmd == "sortedset.zpopmax" {
                "ZPOPMAX"
            } else {
                "ZPOPMIN"
            };
            (
                raw,
                vec![
                    field_str("key"),
                    WireValue::Int(payload["count"].as_u64().unwrap_or(1) as i64),
                ],
            )
        }

        "sortedset.zremrangebyrank" => (
            "ZREMRANGEBYRANK",
            vec![
                field_str("key"),
                WireValue::Int(payload["start"].as_i64().unwrap_or(0)),
                WireValue::Int(payload["stop"].as_i64().unwrap_or(-1)),
            ],
        ),

        "sortedset.zremrangebyscore" => (
            "ZREMRANGEBYSCORE",
            vec![
                field_str("key"),
                WireValue::Str(
                    payload["min"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["min"].as_str().unwrap_or("-inf").to_string()),
                ),
                WireValue::Str(
                    payload["max"]
                        .as_f64()
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| payload["max"].as_str().unwrap_or("+inf").to_string()),
                ),
            ],
        ),

        "sortedset.zinterstore" | "sortedset.zunionstore" | "sortedset.zdiffstore" => {
            let raw: &'static str = match cmd {
                "sortedset.zinterstore" => "ZINTERSTORE",
                "sortedset.zunionstore" => "ZUNIONSTORE",
                _ => "ZDIFFSTORE",
            };
            let mut args = vec![
                field_str("destination"),
                WireValue::Int(
                    payload["keys"]
                        .as_array()
                        .map(|a| a.len() as i64)
                        .unwrap_or(0),
                ),
            ];
            if let Some(keys) = payload["keys"].as_array() {
                for k in keys {
                    args.push(WireValue::Str(k.as_str().unwrap_or("").to_string()));
                }
            }
            (raw, args)
        }

        // Commands with no native mapping fall back to HTTP.
        _ => return None,
    })
}

// ── Response mapper ───────────────────────────────────────────────────────────

/// Convert a raw `WireValue` response into the JSON shape that SDK managers
/// expect from [`SynapClient::send_command`].
pub(crate) fn map_response(cmd: &str, wire: WireValue) -> Value {
    match cmd {
        // ── KV ────────────────────────────────────────────────────────────────
        // kv.get: managers do serde_json::from_value(response)? — pass through.
        "kv.get" => wire.to_json(),
        "kv.set" => json!({}),
        "kv.del" => {
            let n = wire.as_int().unwrap_or(0);
            let deleted = matches!(wire, WireValue::Bool(true)) || n > 0;
            json!({"deleted": deleted})
        }
        "kv.exists" => {
            let n = wire.as_int().unwrap_or(0);
            let exists = matches!(wire, WireValue::Bool(true)) || n > 0;
            json!({"exists": exists})
        }
        "kv.incr" | "kv.decr" => json!({"value": wire.as_int().unwrap_or(0)}),
        "kv.keys" => {
            let keys: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"keys": keys})
        }
        "kv.expire" => json!({}),
        "kv.ttl" => wire.to_json(),

        // ── Hash ──────────────────────────────────────────────────────────────
        "hash.set" => json!({"success": wire.as_int().map(|n| n >= 0).unwrap_or(true)}),
        "hash.get" => {
            let v = if wire.is_null() {
                Value::Null
            } else {
                wire.to_json()
            };
            json!({"value": v})
        }
        "hash.getall" => {
            // HGETALL returns a flat array: [field1, val1, field2, val2, ...]
            let mut fields = serde_json::Map::new();
            if let WireValue::Array(arr) = wire {
                for chunk in arr.chunks(2) {
                    if let (Some(k), Some(v)) = (chunk.first(), chunk.get(1)) {
                        if let Some(key_str) = k.as_str() {
                            fields.insert(key_str.to_string(), v.to_json());
                        }
                    }
                }
            }
            json!({"fields": Value::Object(fields)})
        }
        "hash.del" => {
            let n = wire.as_int().unwrap_or(0);
            let n = if matches!(wire, WireValue::Bool(true)) {
                1
            } else {
                n
            };
            json!({"deleted": n})
        }
        "hash.exists" => {
            let n = wire.as_int().unwrap_or(0);
            let exists = matches!(wire, WireValue::Bool(true)) || n > 0;
            json!({"exists": exists})
        }
        "hash.keys" => {
            let arr: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"fields": arr})
        }
        "hash.values" => {
            let arr: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"values": arr})
        }
        "hash.len" => json!({"length": wire.as_int().unwrap_or(0)}),
        "hash.mset" => json!({"success": !wire.is_null()}),
        "hash.mget" => {
            let values: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"values": values})
        }
        "hash.incrby" => json!({"value": wire.as_int().unwrap_or(0)}),
        "hash.incrbyfloat" => {
            let f = wire
                .as_float()
                .or_else(|| wire.as_str().and_then(|s| s.parse().ok()))
                .unwrap_or(0.0);
            json!({"value": f})
        }
        "hash.setnx" => json!({"created": wire.as_int().unwrap_or(0) > 0}),

        // ── List ──────────────────────────────────────────────────────────────
        "list.lpush" | "list.rpush" | "list.lpushx" | "list.rpushx" => {
            json!({"length": wire.as_int().unwrap_or(0)})
        }
        "list.lpop" | "list.rpop" => {
            let values: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                WireValue::Str(s) => vec![json!(s)],
                WireValue::Bytes(b) => match String::from_utf8(b.clone()) {
                    Ok(s) => vec![json!(s)],
                    Err(_) => vec![json!(b)],
                },
                WireValue::Null => vec![],
                _ => vec![],
            };
            json!({"values": values})
        }
        "list.range" => {
            let values: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"values": values})
        }
        "list.len" => json!({"length": wire.as_int().unwrap_or(0)}),
        "list.index" => wire.to_json(),
        "list.set" | "list.trim" => json!({}),
        "list.rem" => json!({"count": wire.as_int().unwrap_or(0)}),
        "list.insert" => json!({"length": wire.as_int().unwrap_or(-1)}),
        "list.rpoplpush" => wire.to_json(),
        "list.pos" => wire.to_json(),

        // ── Set ───────────────────────────────────────────────────────────────
        "set.add" => json!({"added": wire.as_int().unwrap_or(0)}),
        "set.rem" => json!({"removed": wire.as_int().unwrap_or(0)}),
        "set.ismember" => json!({"is_member": wire.as_int().unwrap_or(0) > 0}),
        "set.members" | "set.pop" | "set.randmember" => {
            let members: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                WireValue::Str(s) => vec![json!(s)],
                WireValue::Null => vec![],
                _ => vec![],
            };
            json!({"members": members})
        }
        "set.card" => json!({"cardinality": wire.as_int().unwrap_or(0)}),
        "set.move" => json!({"moved": wire.as_int().unwrap_or(0) > 0}),
        "set.inter" | "set.union" | "set.diff" => {
            let members: Vec<Value> = match wire {
                WireValue::Array(arr) => arr.iter().map(WireValue::to_json).collect(),
                _ => vec![],
            };
            json!({"members": members})
        }
        "set.interstore" | "set.unionstore" | "set.diffstore" => {
            json!({"count": wire.as_int().unwrap_or(0)})
        }

        // ── Sorted Set ────────────────────────────────────────────────────────
        "sortedset.zadd" => json!({"added": wire.as_int().unwrap_or(0)}),
        "sortedset.zrem" => json!({"removed": wire.as_int().unwrap_or(0)}),
        "sortedset.zscore" => {
            let score = wire
                .as_float()
                .or_else(|| wire.as_str().and_then(|s| s.parse().ok()));
            json!({"score": score})
        }
        "sortedset.zcard" => json!({"count": wire.as_int().unwrap_or(0)}),
        "sortedset.zincrby" => {
            let score = wire
                .as_float()
                .or_else(|| wire.as_str().and_then(|s| s.parse().ok()))
                .unwrap_or(0.0);
            json!({"score": score})
        }
        "sortedset.zrank" | "sortedset.zrevrank" => {
            if wire.is_null() {
                json!({"rank": null})
            } else {
                json!({"rank": wire.as_int().unwrap_or(-1)})
            }
        }
        "sortedset.zcount" | "sortedset.zremrangebyrank" | "sortedset.zremrangebyscore" => {
            json!({"count": wire.as_int().unwrap_or(0)})
        }
        "sortedset.zrange" | "sortedset.zrevrange" | "sortedset.zrangebyscore" => {
            // ZRANGE … WITHSCORES returns interleaved [member, score, ...].
            // Without WITHSCORES returns plain [member, ...].
            // The SDK manager always requests with_scores, so we build ScoredMember objects.
            let members: Vec<Value> = match wire {
                WireValue::Array(arr) => {
                    // Check if interleaved (even count, alternating member/score strings).
                    if arr.len() % 2 == 0 && !arr.is_empty() {
                        arr.chunks(2)
                            .map(|chunk| {
                                let member = chunk[0].as_str().unwrap_or("").to_string();
                                let score = chunk[1]
                                    .as_float()
                                    .or_else(|| chunk[1].as_str().and_then(|s| s.parse().ok()))
                                    .unwrap_or(0.0);
                                json!({"member": member, "score": score})
                            })
                            .collect()
                    } else {
                        // Plain member list — score unknown, default to 0.
                        arr.iter()
                            .map(|v| json!({"member": v.as_str().unwrap_or(""), "score": 0.0}))
                            .collect()
                    }
                }
                _ => vec![],
            };
            json!({"members": members})
        }
        "sortedset.zpopmin" | "sortedset.zpopmax" => {
            // Returns interleaved [member, score, ...].
            let pairs: Vec<Value> = match wire {
                WireValue::Array(arr) => arr
                    .chunks(2)
                    .filter_map(|chunk| {
                        if chunk.len() == 2 {
                            let member = chunk[0].as_str().unwrap_or("").to_string();
                            let score = chunk[1]
                                .as_float()
                                .or_else(|| chunk[1].as_str().and_then(|s| s.parse().ok()))
                                .unwrap_or(0.0);
                            Some(json!({"member": member, "score": score}))
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => vec![],
            };
            json!({"members": pairs})
        }
        "sortedset.zinterstore" | "sortedset.zunionstore" | "sortedset.zdiffstore" => {
            json!({"count": wire.as_int().unwrap_or(0)})
        }

        // Fallthrough: return the raw JSON representation.
        _ => wire.to_json(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wire_value_to_json_basic() {
        assert_eq!(WireValue::Null.to_json(), Value::Null);
        assert_eq!(WireValue::Bool(true).to_json(), json!(true));
        assert_eq!(WireValue::Int(42).to_json(), json!(42));
        assert_eq!(WireValue::Float(1.5).to_json(), json!(1.5));
        assert_eq!(WireValue::Str("hi".into()).to_json(), json!("hi"));
    }

    #[test]
    fn wire_value_bytes_utf8() {
        let v = WireValue::Bytes(b"hello".to_vec());
        assert_eq!(v.to_json(), json!("hello"));
    }

    #[test]
    fn map_command_kv_get() {
        let payload = json!({"key": "mykey"});
        let (cmd, args) = map_command("kv.get", &payload).unwrap();
        assert_eq!(cmd, "GET");
        assert_eq!(args, vec![WireValue::Str("mykey".into())]);
    }

    #[test]
    fn map_command_kv_set_with_ttl() {
        let payload = json!({"key": "k", "value": "v", "ttl": 60});
        let (cmd, args) = map_command("kv.set", &payload).unwrap();
        assert_eq!(cmd, "SET");
        assert_eq!(
            args,
            vec![
                WireValue::Str("k".into()),
                WireValue::Str("v".into()),
                WireValue::Str("EX".into()),
                WireValue::Int(60),
            ]
        );
    }

    #[test]
    fn map_command_kv_set_no_ttl() {
        let payload = json!({"key": "k", "value": "v", "ttl": null});
        let (cmd, args) = map_command("kv.set", &payload).unwrap();
        assert_eq!(cmd, "SET");
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn map_command_unknown_returns_none() {
        let payload = json!({});
        assert!(map_command("queue.enqueue", &payload).is_none());
        assert!(map_command("pubsub.subscribe", &payload).is_none());
        assert!(map_command("stream.publish", &payload).is_none());
    }

    #[test]
    fn map_response_kv_del() {
        let v = map_response("kv.del", WireValue::Int(1));
        assert_eq!(v["deleted"], json!(true));

        let v = map_response("kv.del", WireValue::Int(0));
        assert_eq!(v["deleted"], json!(false));
    }

    #[test]
    fn map_response_kv_exists() {
        let v = map_response("kv.exists", WireValue::Int(1));
        assert_eq!(v["exists"], json!(true));
    }

    #[test]
    fn map_response_hash_getall() {
        let wire = WireValue::Array(vec![
            WireValue::Str("name".into()),
            WireValue::Str("Alice".into()),
            WireValue::Str("age".into()),
            WireValue::Str("30".into()),
        ]);
        let v = map_response("hash.getall", wire);
        assert_eq!(v["fields"]["name"], json!("Alice"));
        assert_eq!(v["fields"]["age"], json!("30"));
    }

    #[test]
    fn map_response_zrange_withscores() {
        let wire = WireValue::Array(vec![
            WireValue::Str("player1".into()),
            WireValue::Str("100.0".into()),
            WireValue::Str("player2".into()),
            WireValue::Str("200.0".into()),
        ]);
        let v = map_response("sortedset.zrange", wire);
        let members = v["members"].as_array().unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0]["member"], json!("player1"));
        assert_eq!(members[0]["score"], json!(100.0));
    }

    #[test]
    fn map_command_hash_mset_hashmap_format() {
        let mut map = serde_json::Map::new();
        map.insert("f1".into(), json!("v1"));
        map.insert("f2".into(), json!("v2"));
        let payload = json!({"key": "k", "fields": Value::Object(map)});
        let (cmd, args) = map_command("hash.mset", &payload).unwrap();
        assert_eq!(cmd, "HSET");
        assert!(args.len() >= 3); // key + at least one field/value pair
    }

    #[test]
    fn map_command_set_add_multi() {
        let payload = json!({"key": "myset", "members": ["a", "b", "c"]});
        let (cmd, args) = map_command("set.add", &payload).unwrap();
        assert_eq!(cmd, "SADD");
        assert_eq!(args.len(), 4); // key + 3 members
    }

    #[test]
    fn wire_value_roundtrip_msgpack() {
        let vals = vec![
            WireValue::Null,
            WireValue::Bool(true),
            WireValue::Int(i64::MAX),
            WireValue::Float(1.5),
            WireValue::Str("hello".into()),
            WireValue::Array(vec![WireValue::Int(1), WireValue::Str("two".into())]),
        ];
        for v in vals {
            let enc = rmp_serde::to_vec(&v).unwrap();
            let dec: WireValue = rmp_serde::from_slice(&enc).unwrap();
            assert_eq!(v, dec);
        }
    }

    // ── WireValue helpers ─────────────────────────────────────────────────────

    #[test]
    fn wire_value_as_str() {
        assert_eq!(WireValue::Str("hello".into()).as_str(), Some("hello"));
        assert_eq!(WireValue::Int(1).as_str(), None);
        assert_eq!(WireValue::Null.as_str(), None);
    }

    #[test]
    fn wire_value_as_int() {
        assert_eq!(WireValue::Int(99).as_int(), Some(99));
        assert_eq!(WireValue::Int(-1).as_int(), Some(-1));
        assert_eq!(WireValue::Str("x".into()).as_int(), None);
        assert_eq!(WireValue::Null.as_int(), None);
    }

    #[test]
    fn wire_value_as_float() {
        assert_eq!(WireValue::Float(3.125).as_float(), Some(3.125));
        // Str that parses as float
        assert_eq!(WireValue::Str("2.5".into()).as_float(), Some(2.5));
        assert_eq!(WireValue::Str("not-a-float".into()).as_float(), None);
        assert_eq!(WireValue::Null.as_float(), None);
    }

    #[test]
    fn wire_value_is_null() {
        assert!(WireValue::Null.is_null());
        assert!(!WireValue::Int(0).is_null());
        assert!(!WireValue::Bool(false).is_null());
    }

    #[test]
    fn wire_value_to_json_array_and_map() {
        let arr = WireValue::Array(vec![WireValue::Int(1), WireValue::Str("two".into())]);
        let j = arr.to_json();
        assert_eq!(j[0], json!(1));
        assert_eq!(j[1], json!("two"));

        let map = WireValue::Map(vec![(WireValue::Str("key".into()), WireValue::Int(42))]);
        let j = map.to_json();
        assert_eq!(j["key"], json!(42));
    }

    #[test]
    fn wire_value_bytes_non_utf8_renders_hex() {
        let v = WireValue::Bytes(vec![0xFF, 0xFE]);
        let j = v.to_json();
        // Non-UTF8 bytes should become a hex string.
        assert!(j.as_str().unwrap().chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ── map_command – additional commands ─────────────────────────────────────

    #[test]
    fn map_command_kv_delete() {
        let payload = json!({"key": "testkey"});
        let (cmd, args) = map_command("kv.del", &payload).unwrap();
        assert_eq!(cmd, "DEL");
        assert_eq!(args, vec![WireValue::Str("testkey".into())]);
    }

    #[test]
    fn map_command_kv_exists() {
        let payload = json!({"key": "testkey"});
        let (cmd, args) = map_command("kv.exists", &payload).unwrap();
        assert_eq!(cmd, "EXISTS");
        assert_eq!(args, vec![WireValue::Str("testkey".into())]);
    }

    #[test]
    fn map_command_kv_incr() {
        let payload = json!({"key": "counter"});
        let (cmd, args) = map_command("kv.incr", &payload).unwrap();
        assert_eq!(cmd, "INCR");
        assert_eq!(args, vec![WireValue::Str("counter".into())]);
    }

    #[test]
    fn map_command_hash_get() {
        let payload = json!({"key": "myhash", "field": "f1"});
        let (cmd, args) = map_command("hash.get", &payload).unwrap();
        assert_eq!(cmd, "HGET");
        assert_eq!(
            args,
            vec![WireValue::Str("myhash".into()), WireValue::Str("f1".into()),]
        );
    }

    #[test]
    fn map_command_list_lpush() {
        let payload = json!({"key": "mylist", "values": ["a", "b"]});
        let (cmd, args) = map_command("list.lpush", &payload).unwrap();
        assert_eq!(cmd, "LPUSH");
        assert_eq!(
            args,
            vec![
                WireValue::Str("mylist".into()),
                WireValue::Str("a".into()),
                WireValue::Str("b".into()),
            ]
        );
    }

    #[test]
    fn map_command_set_add() {
        let payload = json!({"key": "myset", "members": ["x"]});
        let (cmd, args) = map_command("set.add", &payload).unwrap();
        assert_eq!(cmd, "SADD");
        assert_eq!(
            args,
            vec![WireValue::Str("myset".into()), WireValue::Str("x".into()),]
        );
    }

    #[test]
    fn map_command_queue_publish_returns_none() {
        assert!(map_command("queue.publish", &json!({})).is_none());
    }

    #[test]
    fn map_command_stream_publish_returns_none() {
        assert!(map_command("stream.publish", &json!({})).is_none());
    }

    // ── map_response – additional cases ──────────────────────────────────────

    #[test]
    fn map_response_kv_get_passes_through() {
        let wire = WireValue::Str("myvalue".into());
        let v = map_response("kv.get", wire);
        assert_eq!(v, json!("myvalue"));
    }

    #[test]
    fn map_response_kv_get_null_passes_through() {
        let v = map_response("kv.get", WireValue::Null);
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn map_response_kv_del_deleted_false_when_zero() {
        let v = map_response("kv.del", WireValue::Int(0));
        assert_eq!(v["deleted"], json!(false));
    }

    #[test]
    fn map_response_kv_exists_false_when_zero() {
        let v = map_response("kv.exists", WireValue::Int(0));
        assert_eq!(v["exists"], json!(false));
    }

    #[test]
    fn map_response_kv_incr() {
        let v = map_response("kv.incr", WireValue::Int(5));
        assert_eq!(v["value"], json!(5));
    }

    #[test]
    fn map_response_hash_get_value() {
        let v = map_response("hash.get", WireValue::Str("hello".into()));
        assert_eq!(v["value"], json!("hello"));
    }

    #[test]
    fn map_response_hash_get_null() {
        let v = map_response("hash.get", WireValue::Null);
        assert_eq!(v["value"], Value::Null);
    }

    #[test]
    fn map_response_list_lpush_length() {
        let v = map_response("list.lpush", WireValue::Int(3));
        assert_eq!(v["length"], json!(3));
    }

    #[test]
    fn map_response_set_add_added_count() {
        let v = map_response("set.add", WireValue::Int(2));
        assert_eq!(v["added"], json!(2));
    }

    // ── SynapRpcTransport – real TCP round-trip ───────────────────────────────

    /// Start a minimal SynapRPC echo server that handles exactly one request.
    ///
    /// The server reads a 4-byte LE length prefix, deserialises the msgpack
    /// `RpcRequest`, then sends back an `RpcResponse` with `Ok(WireValue::Str("testvalue"))`.
    async fn run_synap_rpc_server_once(
        listener: tokio::net::TcpListener,
        expected_cmd: &'static str,
    ) {
        let (mut stream, _) = listener.accept().await.expect("accept");

        // Read 4-byte length prefix.
        let mut len_buf = [0u8; 4];
        tokio::io::AsyncReadExt::read_exact(&mut stream, &mut len_buf)
            .await
            .expect("read len");
        let body_len = u32::from_le_bytes(len_buf) as usize;

        // Read msgpack body.
        let mut body = vec![0u8; body_len];
        tokio::io::AsyncReadExt::read_exact(&mut stream, &mut body)
            .await
            .expect("read body");

        let req: RpcRequest = rmp_serde::from_slice(&body).expect("decode request");
        assert_eq!(req.command, expected_cmd);

        // Build response.
        let resp = RpcResponse {
            id: req.id,
            result: Ok(WireValue::Str("testvalue".into())),
        };
        let resp_bytes = rmp_serde::to_vec(&resp).expect("encode response");
        let resp_len = (resp_bytes.len() as u32).to_le_bytes();

        tokio::io::AsyncWriteExt::write_all(&mut stream, &resp_len)
            .await
            .expect("write len");
        tokio::io::AsyncWriteExt::write_all(&mut stream, &resp_bytes)
            .await
            .expect("write body");
    }

    #[tokio::test]
    async fn synap_rpc_transport_execute_get() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let port = listener.local_addr().expect("local_addr").port();

        tokio::spawn(run_synap_rpc_server_once(listener, "GET"));

        let transport = SynapRpcTransport::new("127.0.0.1", port, Duration::from_secs(5));
        let result = transport
            .execute("GET", vec![WireValue::Str("testkey".into())])
            .await
            .expect("execute");

        assert_eq!(result, WireValue::Str("testvalue".into()));
    }

    #[tokio::test]
    async fn synap_rpc_transport_command_uppercased() {
        // Verifies that the transport converts the command to uppercase before
        // sending, so the server receives "SET" not "set".
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let port = listener.local_addr().expect("local_addr").port();

        tokio::spawn(run_synap_rpc_server_once(listener, "SET"));

        let transport = SynapRpcTransport::new("127.0.0.1", port, Duration::from_secs(5));
        let _ = transport
            .execute(
                "set",
                vec![WireValue::Str("k".into()), WireValue::Str("v".into())],
            )
            .await
            .expect("execute");
    }

    // ── Resp3Transport – real TCP round-trip ──────────────────────────────────

    /// Start a minimal RESP3 server that responds to HELLO and one GET command.
    async fn run_resp3_server_once(listener: tokio::net::TcpListener) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let (stream, _) = listener.accept().await.expect("accept");
        let (r, mut w) = stream.into_split();
        let mut reader = BufReader::new(r);

        // Read lines and respond to the RESP2 multibulk frame.
        // The transport sends: *2\r\n$3\r\nGET\r\n$7\r\ntestkey\r\n
        // We parse the count line then skip arg lines, replying after each command.
        let mut buf = String::new();
        loop {
            buf.clear();
            let n = reader.read_line(&mut buf).await.expect("read");
            if n == 0 {
                break;
            }
            let line = buf.trim_end_matches(['\r', '\n']);
            if let Some(rest) = line.strip_prefix('*') {
                // Multibulk: read count args then skip them.
                let argc: usize = rest.parse().unwrap_or(0);
                for _ in 0..argc {
                    // Skip "$len\r\n" line.
                    buf.clear();
                    reader.read_line(&mut buf).await.expect("read len line");
                    let len_line = buf.trim_end_matches(['\r', '\n']);
                    if let Some(len_str) = len_line.strip_prefix('$') {
                        let len: usize = len_str.parse().unwrap_or(0);
                        // Read bulk data + CRLF.
                        let mut data = vec![0u8; len + 2];
                        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut data)
                            .await
                            .expect("read bulk");
                    }
                }
                // Respond with a bulk string.
                w.write_all(b"$9\r\ntestvalue\r\n")
                    .await
                    .expect("write response");
                break;
            }
        }
    }

    #[tokio::test]
    async fn resp3_transport_execute_get() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let port = listener.local_addr().expect("local_addr").port();

        tokio::spawn(run_resp3_server_once(listener));

        let transport = Resp3Transport::new("127.0.0.1", port, Duration::from_secs(5));
        let result = transport
            .execute("GET", vec![WireValue::Str("testkey".into())])
            .await
            .expect("execute");

        assert_eq!(result, WireValue::Str("testvalue".into()));
    }

    // ── wire_value_to_resp_bytes ──────────────────────────────────────────────

    #[test]
    fn wire_value_to_resp_bytes_variants() {
        assert_eq!(wire_value_to_resp_bytes(&WireValue::Null), b"".to_vec());
        assert_eq!(
            wire_value_to_resp_bytes(&WireValue::Bool(true)),
            b"1".to_vec()
        );
        assert_eq!(
            wire_value_to_resp_bytes(&WireValue::Bool(false)),
            b"0".to_vec()
        );
        assert_eq!(
            wire_value_to_resp_bytes(&WireValue::Int(42)),
            b"42".to_vec()
        );
        assert_eq!(
            wire_value_to_resp_bytes(&WireValue::Str("hello".into())),
            b"hello".to_vec()
        );
        assert_eq!(
            wire_value_to_resp_bytes(&WireValue::Bytes(vec![1, 2, 3])),
            vec![1u8, 2, 3]
        );
        // Arrays and Maps render as empty bytes.
        assert_eq!(
            wire_value_to_resp_bytes(&WireValue::Array(vec![])),
            b"".to_vec()
        );
        assert_eq!(
            wire_value_to_resp_bytes(&WireValue::Map(vec![])),
            b"".to_vec()
        );
    }
}
