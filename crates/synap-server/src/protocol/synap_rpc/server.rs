//! SynapRPC binary TCP server accept loop.
//!
//! Each connection receives `Request` frames (4-byte LE length + MessagePack
//! body) and responds with `Response` frames in the same format.
//! Multiple in-flight requests per connection are supported via
//! `tokio::spawn` per request.
//!
//! # Metrics collected
//! - `synap_rpc_connections` — active connection gauge
//! - `synap_rpc_commands_total` — per-command counters (ok/err)
//! - `synap_rpc_command_duration_seconds` — per-command latency histogram
//! - `synap_rpc_frame_size_bytes_in` / `synap_rpc_frame_size_bytes_out`
//!
//! # Tracing
//! - Each connection gets a `tracing::info_span!("rpc.conn", peer)`.
//! - Each request gets a `tracing::debug_span!("rpc.req", id, cmd)`.
//! - Commands slower than 1 ms are logged at WARN level.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::core::pubsub::Message as PubSubMessage;
use crate::metrics;
use crate::server::handlers::AppState;

use super::dispatch::dispatch;
use synap_protocol::synap_rpc::codec::{encode_frame, read_request};
use synap_protocol::synap_rpc::types::{Response, SynapValue};

/// Extract a UTF-8 string from a `SynapValue` for the AUTH handshake.
fn rpc_str(v: &SynapValue) -> Option<String> {
    match v {
        SynapValue::Str(s) => Some(s.clone()),
        SynapValue::Bytes(b) => String::from_utf8(b.to_vec()).ok(),
        _ => None,
    }
}

/// Spawn the SynapRPC TCP listener on `addr`.
///
/// Returns immediately; the listener runs as a background task.
pub async fn spawn_synap_rpc_listener(
    state: AppState,
    addr: SocketAddr,
    idle_timeout: std::time::Duration,
    max_connections: usize,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("SynapRPC server listening on {addr}");
    let limiter = Arc::new(tokio::sync::Semaphore::new(max_connections));

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    // Refuse the connection if we are already at capacity.
                    let permit = match Arc::clone(&limiter).try_acquire_owned() {
                        Ok(p) => p,
                        Err(_) => {
                            tracing::warn!(peer = %peer, "SynapRPC max connections reached, refusing");
                            drop(stream);
                            continue;
                        }
                    };
                    metrics::synap_rpc_connection_open();
                    tracing::debug!(peer = %peer, "SynapRPC connection accepted");
                    let state = state.clone();
                    tokio::spawn(async move {
                        let _permit = permit; // released when this connection ends
                        let span = tracing::info_span!("rpc.conn", peer = %peer);
                        let _guard = span.enter();
                        if let Err(e) = handle_connection(stream, state, idle_timeout).await {
                            tracing::debug!(peer = %peer, error = %e, "SynapRPC connection error");
                        }
                        metrics::synap_rpc_connection_close();
                        tracing::debug!(peer = %peer, "SynapRPC connection closed");
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "SynapRPC accept error");
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
    let peer = stream.peer_addr()?;
    // Disable Nagle's algorithm so length-prefixed replies aren't held ~40ms
    // waiting for a delayed ACK (see the RESP3 server for the full rationale).
    let _ = stream.set_nodelay(true);
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // Writer channel: dispatch tasks send (response, metadata) here; a
    // dedicated writer task serialises them to the socket in arrival order.
    let (tx, mut rx) = mpsc::channel::<(Response, String, f64, usize)>(64);
    // Buffer the write half so a pipelined burst of responses is coalesced into
    // one syscall: the writer drains everything already queued, then flushes once
    // (mirrors the RESP3 server). A raw write half would emit one `write()` per
    // response and cap pipelined throughput.
    let mut writer = BufWriter::new(write_half);

    // Writer task — receives (response, command, elapsed_secs, in_frame_bytes).
    let write_task = tokio::spawn(async move {
        // Encode + buffer one response; returns false on a fatal write error.
        async fn emit(
            writer: &mut BufWriter<tokio::net::tcp::OwnedWriteHalf>,
            response: Response,
            command: String,
            elapsed: f64,
            in_bytes: usize,
        ) -> bool {
            let is_err = response.result.is_err();
            match encode_frame(&response) {
                Ok(frame) => {
                    let out_bytes = frame.len();
                    if let Err(e) = writer.write_all(&frame).await {
                        tracing::debug!(error = %e, "SynapRPC write error");
                        return false;
                    }
                    metrics::record_synap_rpc_command(&command, !is_err, elapsed);
                    metrics::synap_rpc_frame_sizes(in_bytes, out_bytes);
                    if elapsed > 0.001 {
                        tracing::warn!(cmd = %command, elapsed_ms = elapsed * 1_000.0, "SynapRPC slow command");
                    }
                    true
                }
                Err(e) => {
                    tracing::error!(cmd = %command, error = %e, "SynapRPC encode error");
                    true
                }
            }
        }

        'outer: while let Some((response, command, elapsed, in_bytes)) = rx.recv().await {
            if !emit(&mut writer, response, command, elapsed, in_bytes).await {
                break;
            }
            // Drain any responses already queued (a pipelined burst) before flushing.
            while let Ok((response, command, elapsed, in_bytes)) = rx.try_recv() {
                if !emit(&mut writer, response, command, elapsed, in_bytes).await {
                    break 'outer;
                }
            }
            if let Err(e) = writer.flush().await {
                tracing::debug!(error = %e, "SynapRPC flush error");
                break;
            }
        }
    });

    // Shared state — Arc so each request task can clone it cheaply.
    let state = Arc::new(state);

    // Per-connection auth flag + resolved user (for per-command ACL, phase6h).
    // The read loop is sequential, so AUTH and the gate checks below serialize
    // ahead of the per-request tasks.
    let mut authenticated = !state.require_auth;
    let mut auth_user: Option<crate::auth::User> = None;

    // Read loop — one task per request for concurrency.
    loop {
        // Read the next frame, bounded by the idle timeout (slow-loris
        // resistance, phase6i). A zero timeout disables the bound.
        let read = if idle_timeout.is_zero() {
            read_request(&mut reader).await
        } else {
            match tokio::time::timeout(idle_timeout, read_request(&mut reader)).await {
                Ok(r) => r,
                Err(_) => {
                    tracing::debug!(peer = %peer, "SynapRPC idle timeout, closing connection");
                    break;
                }
            }
        };
        let req = match read {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break, // clean EOF
            Err(e) => {
                tracing::debug!(peer = %peer, error = %e, "SynapRPC read error");
                break;
            }
        };

        // Approximate in-frame size: rmp_serde encoding of the request.
        let in_bytes = rmp_serde::to_vec(&req).map(|b| b.len() + 4).unwrap_or(0);
        let command = req.command.clone();

        // AUTH is handled inline (serialized ahead of request tasks).
        // `AUTH <password>` (default user) or `AUTH <user> <password>`.
        if req.command.eq_ignore_ascii_case("AUTH") {
            let creds: Option<(String, String)> = match req.args.as_slice() {
                [p] => rpc_str(p).map(|p| ("default".to_string(), p)),
                [u, p, ..] => match (rpc_str(u), rpc_str(p)) {
                    (Some(u), Some(p)) => Some((u, p)),
                    _ => None,
                },
                _ => None,
            };
            let resolved = match (&creds, &state.user_manager) {
                (Some((u, p)), Some(um)) => um.authenticate(u, p).ok(),
                _ => None,
            };
            let response = if let Some(user) = resolved {
                authenticated = true;
                auth_user = Some(user);
                Response::ok(req.id, SynapValue::Str("OK".into()))
            } else {
                Response::err(
                    req.id,
                    "WRONGPASS invalid username-password pair or user is disabled.".to_string(),
                )
            };
            let _ = tx.send((response, command, 0.0, in_bytes)).await;
            continue;
        }

        // Reject every other command until the connection has authenticated.
        if !authenticated {
            let response = Response::err(req.id, "NOAUTH Authentication required.".to_string());
            let _ = tx.send((response, command, 0.0, in_bytes)).await;
            continue;
        }

        // Per-command ACL (phase6h): destructive/admin commands require an admin
        // user when auth is enforced. With auth disabled the port is trusted.
        if state.require_auth && crate::auth::command_requires_admin(&req.command) {
            let is_admin = auth_user.as_ref().map(|u| u.is_admin).unwrap_or(false);
            if !is_admin {
                let response = Response::err(
                    req.id,
                    "NOPERM this command requires admin privileges".to_string(),
                );
                let _ = tx.send((response, command, 0.0, in_bytes)).await;
                continue;
            }
        }

        let state = Arc::clone(&state);
        let tx = tx.clone();

        tokio::spawn(async move {
            let start = Instant::now();
            let is_subscribe = req.command == "SUBSCRIBE";
            let span = tracing::debug_span!("rpc.req", id = req.id, cmd = %req.command);
            let response = {
                let _g = span.enter();
                dispatch(&state, req).await
            };
            let elapsed = start.elapsed().as_secs_f64();

            // After SUBSCRIBE succeeds, register the connection's write channel
            // with the pubsub router so that publish() delivers push frames.
            if is_subscribe {
                if let Ok(SynapValue::Map(ref pairs)) = response.result {
                    let sub_id = pairs.iter().find_map(|(k, v)| {
                        if k.as_str() == Some("subscriber_id") {
                            v.as_str().map(str::to_owned)
                        } else {
                            None
                        }
                    });
                    if let (Some(sub_id), Some(router)) = (sub_id, state.pubsub_router.as_ref()) {
                        let router = Arc::clone(router);
                        let (push_tx, mut push_rx) = tokio::sync::mpsc::channel::<PubSubMessage>(
                            crate::core::pubsub::SUBSCRIBER_CHANNEL_CAPACITY,
                        );
                        router.register_connection(sub_id, push_tx);
                        let tx_push = tx.clone();
                        tokio::spawn(async move {
                            while let Some(msg) = push_rx.recv().await {
                                let push_value = SynapValue::Map(vec![
                                    (SynapValue::Str("topic".into()), SynapValue::Str(msg.topic)),
                                    (
                                        SynapValue::Str("payload".into()),
                                        SynapValue::Str(msg.payload.to_string()),
                                    ),
                                    (SynapValue::Str("id".into()), SynapValue::Str(msg.id)),
                                    (
                                        SynapValue::Str("timestamp".into()),
                                        SynapValue::Int(msg.timestamp as i64),
                                    ),
                                ]);
                                let push_response = Response {
                                    id: u32::MAX,
                                    result: Ok(push_value),
                                };
                                if tx_push
                                    .send((push_response, "_push".to_string(), 0.0, 0))
                                    .await
                                    .is_err()
                                {
                                    break; // writer task gone — connection closed
                                }
                            }
                        });
                    }
                }
            }

            let _ = tx.send((response, command, elapsed, in_bytes)).await;
        });
    }

    // Drop sender so the writer task can finish.
    drop(tx);
    let _ = write_task.await;

    tracing::debug!(peer = %peer, "SynapRPC connection closed");
    Ok(())
}
