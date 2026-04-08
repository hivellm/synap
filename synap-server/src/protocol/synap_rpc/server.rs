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

use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::metrics;
use crate::server::handlers::AppState;

use super::codec::{encode_frame, read_request, write_response};
use super::dispatch::dispatch;
use super::types::Response;

/// Spawn the SynapRPC TCP listener on `addr`.
///
/// Returns immediately; the listener runs as a background task.
pub async fn spawn_synap_rpc_listener(state: AppState, addr: SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("SynapRPC server listening on {addr}");

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    metrics::synap_rpc_connection_open();
                    tracing::debug!(peer = %peer, "SynapRPC connection accepted");
                    let state = state.clone();
                    tokio::spawn(async move {
                        let span = tracing::info_span!("rpc.conn", peer = %peer);
                        let _guard = span.enter();
                        if let Err(e) = handle_connection(stream, state).await {
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

async fn handle_connection(stream: TcpStream, state: AppState) -> std::io::Result<()> {
    let peer = stream.peer_addr()?;
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // Writer channel: dispatch tasks send (response, metadata) here; a
    // dedicated writer task serialises them to the socket in arrival order.
    let (tx, mut rx) = mpsc::channel::<(Response, String, f64, usize)>(64);
    let mut writer = write_half;

    // Writer task — receives (response, command, elapsed_secs, in_frame_bytes).
    let write_task = tokio::spawn(async move {
        while let Some((response, command, elapsed, in_bytes)) = rx.recv().await {
            let is_err = response.result.is_err();

            // Encode and write the response.
            match encode_frame(&response) {
                Ok(frame) => {
                    let out_bytes = frame.len();
                    if let Err(e) = write_response(&mut writer, &response).await {
                        tracing::debug!(error = %e, "SynapRPC write error");
                        break;
                    }
                    // Record metrics after successful write.
                    metrics::record_synap_rpc_command(&command, !is_err, elapsed);
                    metrics::synap_rpc_frame_sizes(in_bytes, out_bytes);

                    if elapsed > 0.001 {
                        tracing::warn!(
                            cmd = %command,
                            elapsed_ms = elapsed * 1_000.0,
                            "SynapRPC slow command"
                        );
                    } else {
                        tracing::debug!(
                            cmd = %command,
                            elapsed_us = elapsed * 1_000_000.0,
                            ok = !is_err,
                            "SynapRPC command"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(cmd = %command, error = %e, "SynapRPC encode error");
                }
            }
        }
    });

    // Shared state — Arc so each request task can clone it cheaply.
    let state = Arc::new(state);

    // Read loop — one task per request for concurrency.
    loop {
        // Read raw frame bytes so we can measure the incoming size.
        let req = match read_request(&mut reader).await {
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
        let state = Arc::clone(&state);
        let tx = tx.clone();

        tokio::spawn(async move {
            let start = Instant::now();
            let span = tracing::debug_span!("rpc.req", id = req.id, cmd = %req.command);
            let response = {
                let _g = span.enter();
                dispatch(&state, req).await
            };
            let elapsed = start.elapsed().as_secs_f64();
            let _ = tx.send((response, command, elapsed, in_bytes)).await;
        });
    }

    // Drop sender so the writer task can finish.
    drop(tx);
    let _ = write_task.await;

    tracing::debug!(peer = %peer, "SynapRPC connection closed");
    Ok(())
}
