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

use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

use serde_json::Value;

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

// ── Shared wire types (single source of truth) ────────────────────────────────
//
// The value model, `Request`/`Response` and the frame codec belong to
// `thunder` — the family's shared binary RPC crate, which the Synap server also
// runs on, so the two halves cannot drift. `WireValue` stays a crate-local
// alias so existing call sites keep reading the same way; the two Synap-only
// conveniences live in [`value_ext`].
pub(crate) use thunder::Value as WireValue;

pub(crate) mod value_ext;
pub(crate) use value_ext::WireValueExt;

/// Synap's protocol configuration, mirroring the server's `synap_config()`:
/// `AUTH`-command handshake, push enabled, RESP3-style error prefixes.
///
/// Declared here rather than imported so the SDK depends only on registry
/// crates — `cargo publish` rejects path dependencies, and a Synap SDK that
/// dragged the server crate to crates.io is exactly what Thunder dissolved.
fn synap_protocol_config() -> thunder::Config {
    use thunder::wire::config::{ErrorConvention, Handshake, HelloStyle, PushPolicy};
    thunder::Config::standard()
        .scheme("synap")
        .port(15501)
        .handshake(Handshake::AuthCommand)
        .hello_style(HelloStyle::NotUsed)
        .push(PushPolicy::Enabled)
        .error_codes(ErrorConvention::Resp3Prefixes)
        .max_frame_bytes(512 * 1024 * 1024)
}

/// Credentials for the RPC handshake, resolved from [`SynapConfig`].
#[derive(Debug, Clone)]
pub(crate) enum RpcCredentials {
    /// Bearer token — the same `auth_token` the HTTP transport sends.
    Token(String),
    /// `AUTH <user> <pass>`.
    UserPass(String, String),
}

/// Map a Thunder client error onto the SDK's error type, preserving the
/// distinctions callers already match on.
fn map_client_error(err: thunder::ClientError) -> SynapError {
    use thunder::ClientError;
    match err {
        ClientError::Timeout => SynapError::Timeout,
        ClientError::Auth { message } => SynapError::Unauthorized(message),
        // The raw server string, verbatim — callers match on the `NOPERM` /
        // `ERR …` prefixes exactly as they did before the swap.
        ClientError::Server { message, .. } => SynapError::ServerError(message),
        other => SynapError::Other(format!("SynapRPC: {other}")),
    }
}

// ── SynapRPC transport ────────────────────────────────────────────────────────

/// Multiplexed connection to the SynapRPC listener, backed by Thunder's client.
///
/// One persistent TCP connection carries every request: calls are demultiplexed
/// by frame id, so concurrent commands pipeline instead of queueing behind a
/// mutex. Connect and per-call timeouts, the frame cap, lazy reconnect and
/// typed auth errors all come from Thunder.
pub(crate) struct SynapRpcTransport {
    endpoint: String,
    client_config: thunder::ClientConfig,
    /// Connected on first use — `new` is sync, dialing is not.
    client: Mutex<Option<Arc<thunder::Client>>>,
}

impl SynapRpcTransport {
    pub(crate) fn new(
        host: &str,
        port: u16,
        timeout: Duration,
        credentials: Option<RpcCredentials>,
    ) -> Self {
        let mut client_config = thunder::ClientConfig::new()
            .connect_timeout(timeout)
            .call_timeout(timeout)
            .client_name("synap-rust-sdk");

        // Unlike the pre-Thunder transport, which never authenticated on the
        // RPC port at all, credentials now travel in the connection handshake.
        client_config = match credentials {
            Some(RpcCredentials::Token(token)) => client_config.token(token),
            Some(RpcCredentials::UserPass(user, pass)) => client_config.user_pass(user, pass),
            None => client_config,
        };

        Self {
            endpoint: format!("synap://{host}:{port}"),
            client_config,
            client: Mutex::new(None),
        }
    }

    /// Dial a fresh Thunder client against the configured endpoint.
    async fn dial(&self) -> Result<Arc<thunder::Client>> {
        thunder::Client::connect_with(
            &self.endpoint,
            synap_protocol_config(),
            self.client_config.clone(),
        )
        .await
        .map(Arc::new)
        .map_err(map_client_error)
    }

    /// The shared client, dialed on first use and replaced if it died.
    ///
    /// Thunder reconnects lazily within a live client; this only covers the
    /// case where the client itself was poisoned beyond recovery.
    async fn client(&self) -> Result<Arc<thunder::Client>> {
        let mut guard = self.client.lock().await;
        if let Some(existing) = guard.as_ref()
            && existing.is_alive()
        {
            return Ok(Arc::clone(existing));
        }
        let fresh = self.dial().await?;
        *guard = Some(Arc::clone(&fresh));
        Ok(fresh)
    }

    /// Send `cmd ARGS…` and return the response value.
    pub(crate) async fn execute(&self, cmd: &str, args: Vec<WireValue>) -> Result<WireValue> {
        let client = self.client().await?;
        client
            .call(cmd.to_ascii_uppercase(), args)
            .await
            .map_err(map_client_error)
    }

    /// Open a dedicated connection, send SUBSCRIBE, and relay server-push
    /// frames (`id == PUSH_ID`) to the returned subscription.
    ///
    /// The connection is dedicated so a subscription's push hook never competes
    /// with the request path, and it is owned by the returned
    /// [`PushSubscription`] — dropping that closes the socket and ends the
    /// reader task, with no keeper task or liveness polling in between.
    pub(crate) async fn subscribe_push(&self, topics: Vec<String>) -> Result<PushSubscription> {
        let client = self.dial().await?;

        // Register the hook before SUBSCRIBE, so a message published between
        // the server's reply and the registration cannot slip past.
        let (push_tx, messages) = tokio::sync::mpsc::unbounded_channel::<Value>();
        client.on_push(move |value| {
            let _ = push_tx.send(value.to_json());
        });

        let result = client
            .call(
                "SUBSCRIBE",
                topics.into_iter().map(WireValue::Str).collect(),
            )
            .await
            .map_err(map_client_error)?;

        let subscriber_id = result
            .map_get("subscriber_id")
            .and_then(WireValue::as_str)
            .unwrap_or_default()
            .to_owned();

        Ok(PushSubscription {
            subscriber_id,
            messages,
            _client: client,
        })
    }
}

/// A live SUBSCRIBE, with the connection that serves it.
///
/// `messages` yields each push frame as a `serde_json::Value` with fields
/// `topic`, `payload`, `id` and `timestamp`. Dropping the subscription drops
/// the client, which closes the connection — so the subscription's lifetime is
/// the connection's lifetime, stated in the type rather than managed by hand.
pub(crate) struct PushSubscription {
    pub(crate) subscriber_id: String,
    pub(crate) messages: tokio::sync::mpsc::UnboundedReceiver<Value>,
    _client: Arc<thunder::Client>,
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
        WireValue::Bytes(b) => b.to_vec(),
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
