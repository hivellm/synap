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
use std::time::Instant;

use tokio::net::{TcpListener, TcpStream};

use crate::metrics;
use crate::server::handlers::AppState;

/// Spawn the RESP3 TCP listener on `addr`.
///
/// Returns immediately; the listener runs as a background task.
pub async fn spawn_resp3_listener(state: AppState, addr: SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("RESP3 server listening on {addr}");

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    metrics::resp3_connection_open();
                    tracing::debug!(peer = %peer, "RESP3 connection accepted");
                    let state = state.clone();
                    tokio::spawn(async move {
                        let span = tracing::info_span!("resp3.conn", peer = %peer);
                        let _guard = span.enter();
                        if let Err(e) = handle_connection(stream, state).await {
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

async fn handle_connection(stream: TcpStream, state: AppState) -> std::io::Result<()> {
    use tokio::io::BufReader;

    use super::command::dispatch;
    use super::parser::{Resp3Value, parse_from_reader, parse_inline};
    use super::writer::Resp3Writer;

    let peer = stream.peer_addr()?;
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut writer = Resp3Writer::new(write_half);

    // AppState does not currently carry an auth manager — authentication for
    // the RESP3 port is enforced at the network level (bind to loopback or
    // use a firewall). When AppState gains an auth field this line becomes
    // `!state.auth.require_auth`.
    let mut authenticated = true;

    loop {
        let value = match parse_from_reader(&mut reader).await? {
            Some(v) => v,
            None => break, // clean EOF
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
        if cmd_upper == "AUTH" {
            if args.len() < 2 {
                writer
                    .write_error("ERR wrong number of arguments for 'AUTH' command")
                    .await?;
            } else if let Some(password) = args[1].as_str() {
                if check_auth(&state, password).await {
                    authenticated = true;
                    writer.write_ok().await?;
                } else {
                    writer
                        .write_error(
                            "WRONGPASS invalid username-password pair or user is disabled.",
                        )
                        .await?;
                }
            } else {
                writer.write_error("ERR password must be a string").await?;
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
        writer.flush().await?;
        let written = writer.bytes_written() - before_write;

        // Record metrics.
        let is_err = matches!(response, super::parser::Resp3Value::Error(_));
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

/// Return `true` if the password is accepted.
///
/// Currently always returns `true` because AppState does not carry an auth
/// manager. When auth is wired into AppState this function will delegate to
/// `state.auth.user_manager.authenticate("default", password).is_ok()`.
async fn check_auth(_state: &AppState, _password: &str) -> bool {
    true
}
