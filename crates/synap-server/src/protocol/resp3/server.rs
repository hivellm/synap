//! RESP3 TCP server accept loop.
//!
//! Listens on a TCP port and handles Redis-compatible RESP3 clients.
//! Each connection runs in its own Tokio task.
//!
//! # Metrics collected
//! - `synap_resp3_connections` — active connection gauge
//! - `synap_resp3_commands_total` — per-command counters (ok/err)
//! - `synap_resp3_command_duration_seconds` — per-command latency histogram
//! - `synap_resp3_bytes_read_total` / `synap_resp3_bytes_written_total`
//!
//! # Tracing
//! - Each connection gets a `tracing::info_span!("resp3.conn", peer)`.
//! - Each command gets a `tracing::debug_span!("resp3.cmd", cmd)`.
//! - Commands slower than 1 ms are logged at WARN level.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

use crate::metrics;
use crate::server::handlers::AppState;

/// Maximum concurrent client connections accepted per binary listener. Beyond
/// this the listener refuses new connections instead of exhausting FDs/memory
/// (audit M-015).
pub const MAX_CONNECTIONS: usize = 10_000;

/// Spawn the RESP3 TCP listener on `addr`.
///
/// Returns immediately; the listener runs as a background task.
pub async fn spawn_resp3_listener(
    state: AppState,
    addr: SocketAddr,
    idle_timeout: std::time::Duration,
    max_connections: usize,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("RESP3 server listening on {addr}");
    let limiter = Arc::new(Semaphore::new(max_connections));

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    // Refuse the connection if we are already at capacity.
                    let permit = match Arc::clone(&limiter).try_acquire_owned() {
                        Ok(p) => p,
                        Err(_) => {
                            tracing::warn!(peer = %peer, "RESP3 max connections reached, refusing");
                            drop(stream);
                            continue;
                        }
                    };
                    metrics::resp3_connection_open();
                    tracing::debug!(peer = %peer, "RESP3 connection accepted");
                    let state = state.clone();
                    tokio::spawn(async move {
                        let _permit = permit; // released when this connection ends
                        let span = tracing::info_span!("resp3.conn", peer = %peer);
                        let _guard = span.enter();
                        if let Err(e) = handle_connection(stream, state, idle_timeout).await {
                            tracing::debug!(peer = %peer, error = %e, "RESP3 connection error");
                        }
                        metrics::resp3_connection_close();
                        tracing::debug!(peer = %peer, "RESP3 connection closed");
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "RESP3 accept error");
                }
            }
        }
    });

    Ok(())
}

async fn handle_connection(
    stream: TcpStream,
    state: AppState,
    idle_timeout: std::time::Duration,
) -> std::io::Result<()> {
    use tokio::io::BufReader;

    use super::command::dispatch;
    use synap_protocol::resp3::parser::{Resp3Value, parse_from_reader, parse_inline};
    use synap_protocol::resp3::writer::Resp3Writer;

    let peer = stream.peer_addr()?;
    // Disable Nagle's algorithm (like Redis). A bulk reply is written as several
    // small segments (length header, payload, CRLF); with Nagle on, the payload
    // segment stalls ~40ms waiting for the header's delayed ACK, capping GET/
    // LRANGE throughput at ~1k rps while single-segment writes (SET/INCR) are
    // unaffected.
    let _ = stream.set_nodelay(true);
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    // Buffer the write half so the pipeline-aware flush actually coalesces a
    // pipelined batch into one syscall — a raw `TcpStream` write half is
    // unbuffered, so without this every bulk-reply segment is its own `write()`
    // and `flush()` is a no-op, capping pipelined throughput.
    let mut writer = Resp3Writer::new(tokio::io::BufWriter::new(write_half));

    // When auth is required the connection starts unauthenticated and must issue
    // a successful AUTH before any command is accepted. `auth_user` holds the
    // resolved user once authenticated, for per-command ACL (phase6h).
    let mut authenticated = !state.require_auth;
    let mut auth_user: Option<crate::auth::User> = None;

    loop {
        // Read the next frame, bounded by the idle timeout (slow-loris
        // resistance, phase6i). A zero timeout disables the bound.
        let read_result = if idle_timeout.is_zero() {
            parse_from_reader(&mut reader).await
        } else {
            match tokio::time::timeout(idle_timeout, parse_from_reader(&mut reader)).await {
                Ok(r) => r,
                Err(_) => {
                    tracing::debug!(peer = %peer, "RESP3 idle timeout, closing connection");
                    break;
                }
            }
        };
        let value = match read_result? {
            Some(v) => v,
            None => {
                // Deliver any responses deferred by the pipeline-aware flush
                // before closing (client may have half-closed its write side
                // while waiting to read the batch).
                writer.flush().await?;
                break; // clean EOF
            }
        };

        // Unwrap inline commands (from redis-cli / telnet).
        let args: Vec<Resp3Value> = match value {
            Resp3Value::Array(a) => a,
            Resp3Value::SimpleString(s) => match parse_inline(&s) {
                Resp3Value::Array(a) => a,
                other => vec![other],
            },
            _other => {
                writer.write_error("ERR unexpected value type").await?;
                writer.flush().await?;
                continue;
            }
        };

        if args.is_empty() {
            writer.write_error("ERR empty command").await?;
            writer.flush().await?;
            continue;
        }

        let cmd_upper = args[0]
            .as_str()
            .map(|s| s.to_ascii_uppercase())
            .unwrap_or_default();

        // AUTH command — handled before dispatch.
        // Redis AUTH: `AUTH <password>` (default user) or `AUTH <user> <password>`.
        if cmd_upper == "AUTH" {
            let creds: Option<(&str, &str)> = if args.len() == 2 {
                args[1].as_str().map(|p| ("default", p))
            } else if args.len() >= 3 {
                match (args[1].as_str(), args[2].as_str()) {
                    (Some(u), Some(p)) => Some((u, p)),
                    _ => None,
                }
            } else {
                writer
                    .write_error("ERR wrong number of arguments for 'AUTH' command")
                    .await?;
                writer.flush().await?;
                continue;
            };

            match creds {
                Some((user, password)) => match check_auth(&state, user, password).await {
                    Some(u) => {
                        auth_user = Some(u);
                        authenticated = true;
                        writer.write_ok().await?;
                    }
                    None => {
                        writer
                            .write_error(
                                "WRONGPASS invalid username-password pair or user is disabled.",
                            )
                            .await?;
                    }
                },
                None => {
                    writer.write_error("ERR password must be a string").await?;
                }
            }
            writer.flush().await?;
            continue;
        }

        // Reject unauthenticated commands (except QUIT/HELLO).
        if !authenticated && cmd_upper != "QUIT" && cmd_upper != "HELLO" {
            writer.write_noauth().await?;
            writer.flush().await?;
            continue;
        }

        // QUIT — close connection.
        if cmd_upper == "QUIT" {
            writer.write_ok().await?;
            writer.flush().await?;
            break;
        }

        // HELLO — RESP3 handshake.
        if cmd_upper == "HELLO" {
            let hello_response = Resp3Value::Map(vec![
                (
                    Resp3Value::SimpleString("server".into()),
                    Resp3Value::SimpleString("synap".into()),
                ),
                (
                    Resp3Value::SimpleString("version".into()),
                    Resp3Value::SimpleString("1.0.0".into()),
                ),
                (
                    Resp3Value::SimpleString("proto".into()),
                    Resp3Value::Integer(3),
                ),
            ]);
            writer.write(&hello_response).await?;
            writer.flush().await?;
            continue;
        }

        // Per-command ACL (phase6h): destructive/admin commands require an admin
        // user when auth is enforced. With auth disabled the binary port is
        // trusted (loopback by default), so no restriction is applied.
        if state.require_auth && crate::auth::command_requires_admin(&cmd_upper) {
            let is_admin = auth_user.as_ref().map(|u| u.is_admin).unwrap_or(false);
            if !is_admin {
                writer
                    .write_error("NOPERM this command requires admin privileges")
                    .await?;
                writer.flush().await?;
                continue;
            }
        }

        // ── Dispatch with timing ─────────────────────────────────────────────
        let start = Instant::now();
        let cmd_span = tracing::debug_span!("resp3.cmd", cmd = %cmd_upper, peer = %peer);
        let response = {
            let _g = cmd_span.enter();
            dispatch(&state, &args).await
        };
        let elapsed = start.elapsed().as_secs_f64();

        // Write response and measure bytes.
        let before_write = writer.bytes_written();
        writer.write(&response).await?;
        // Pipeline-aware flush: only flush once the client's already-buffered
        // commands are drained, so a pipelined batch (e.g. redis-benchmark -P N)
        // is written in a single syscall instead of one flush per command. When
        // the buffer still holds more commands we defer the flush and loop to
        // process them; the next parse consumes from the buffer without awaiting
        // the socket, so a client waiting on responses can never deadlock.
        if reader.buffer().is_empty() {
            writer.flush().await?;
        }
        let written = writer.bytes_written() - before_write;

        // Record metrics.
        let is_err = matches!(
            response,
            synap_protocol::resp3::parser::Resp3Value::Error(_)
        );
        metrics::record_resp3_command(&cmd_upper, !is_err, elapsed);
        metrics::resp3_bytes(0, written); // read bytes tracked per-frame below

        // Slow-command warning (threshold: 1 ms).
        if elapsed > 0.001 {
            tracing::warn!(
                cmd = %cmd_upper,
                peer = %peer,
                elapsed_ms = elapsed * 1_000.0,
                "RESP3 slow command"
            );
        } else {
            tracing::debug!(
                cmd = %cmd_upper,
                peer = %peer,
                elapsed_us = elapsed * 1_000_000.0,
                ok = !is_err,
                "RESP3 command"
            );
        }
    }

    tracing::debug!(peer = %peer, "RESP3 connection closed");
    Ok(())
}

/// Return `true` if the username/password pair authenticates against the
/// configured user manager. Returns `false` when no user manager is present
/// (callers only reach this path when `require_auth` is set).
/// Authenticate and return the resolved user on success (for per-command ACL).
async fn check_auth(state: &AppState, username: &str, password: &str) -> Option<crate::auth::User> {
    match &state.user_manager {
        Some(um) => um.authenticate(username, password).ok(),
        None => None,
    }
}
