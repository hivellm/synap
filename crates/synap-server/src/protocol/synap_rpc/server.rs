//! SynapRPC listener — Synap's command catalog on Thunder's transport.
//!
//! The accept loop, per-connection writer task, frame codec, session state
//! machine and graceful drain all belong to [`thunder::server`]. What lives
//! here is the [`Dispatch`] implementation that binds Synap's engine to it:
//! command routing, credential validation, the per-command ACL, the SUBSCRIBE
//! push bridge, and the Prometheus export.
//!
//! # Metrics collected
//! - `synap_rpc_connections` — active connection gauge
//! - `synap_rpc_connections_refused_total` — accepts refused at the ceiling
//! - `synap_rpc_commands_total` — per-command counters (ok/err)
//! - `synap_rpc_command_duration_seconds` — per-command latency histogram
//! - `synap_rpc_frame_size_bytes_in` / `synap_rpc_frame_size_bytes_out`
//!
//! All of them are fed by [`SynapMetrics`], Thunder's `MetricsObserver` hook,
//! which fires where the listener records its own counters — after the response
//! has left the socket, with the frame sizes the codec already measured.
//!
//! # Tracing
//! - Each request gets a `tracing::debug_span!("rpc.req", cmd)`.
//! - Commands slower than 1 ms are logged at WARN level.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use thunder::server::{
    AuthError, Credentials, Dispatch, ListenerConfig, ListenerHandle, MetricsObserver, Principal,
    ServerInfo, Session, spawn_listener,
};

use crate::auth::User;
use crate::core::pubsub::Message as PubSubMessage;
use crate::metrics;
use crate::server::handlers::AppState;

use super::dispatch::run_command;
use super::{SynapValue, synap_config};

/// Commands slower than this are logged at WARN and counted as slow.
const SLOW_COMMAND_THRESHOLD: Duration = Duration::from_millis(1);

/// Bridges Thunder's listener counters onto Synap's Prometheus registry.
///
/// Thunder calls this at the same point it records its own series — after a
/// successful write — so the two never disagree, and the per-command label and
/// per-frame sizes stay available (they are not reconstructible from totals).
struct SynapMetrics;

impl MetricsObserver for SynapMetrics {
    fn command_completed(
        &self,
        command: &str,
        in_bytes: usize,
        out_bytes: usize,
        duration: Duration,
        is_error: bool,
    ) {
        metrics::record_synap_rpc_command(command, !is_error, duration.as_secs_f64());
        metrics::synap_rpc_frame_sizes(in_bytes, out_bytes);
        if duration > SLOW_COMMAND_THRESHOLD {
            tracing::warn!(
                cmd = %command,
                elapsed_ms = duration.as_secs_f64() * 1_000.0,
                "SynapRPC slow command"
            );
        }
    }

    fn connection_opened(&self) {
        metrics::synap_rpc_connection_open();
    }

    fn connection_closed(&self) {
        metrics::synap_rpc_connection_close();
    }

    fn connection_refused(&self) {
        metrics::synap_rpc_connection_refused();
    }

    fn push_emitted(&self, out_bytes: usize) {
        metrics::synap_rpc_frame_sizes(0, out_bytes);
    }
}

/// Synap's product integration with Thunder (SRV-020): one trait, three hooks.
struct SynapDispatch {
    state: Arc<AppState>,
}

impl Dispatch for SynapDispatch {
    /// The user record resolved once at `AUTH` and carried on the session, so
    /// the per-command ACL reads memory instead of re-querying the store — and
    /// judges the identity captured at authentication rather than whatever the
    /// store holds later.
    type Identity = User;

    async fn dispatch(
        &self,
        session: &Session<User>,
        command: &str,
        args: Vec<SynapValue>,
    ) -> Result<SynapValue, String> {
        // Per-command ACL (phase6h): destructive/admin commands require an
        // admin user when auth is enforced. With auth disabled the port is
        // trusted. Thunder has already gated un-authenticated sessions.
        if self.state.require_auth && crate::auth::command_requires_admin(command) {
            let is_admin = session.with_principal(|p| p.is_some_and(|p| p.identity.is_admin));
            if !is_admin {
                return Err("NOPERM this command requires admin privileges".to_string());
            }
        }

        let result = {
            let span = tracing::debug_span!("rpc.req", cmd = %command);
            let _guard = span.enter();
            run_command(&self.state, command, args).await
        };

        // After SUBSCRIBE / KV.WATCH succeeds, bridge the connection's push
        // channel to the pubsub router so publish() reaches this client.
        if (command.eq_ignore_ascii_case("SUBSCRIBE") || command.eq_ignore_ascii_case("KV.WATCH"))
            && let Ok(ref value) = result
        {
            self.bridge_subscription(session, value);
        }

        result
    }

    async fn authenticate(&self, creds: Credentials) -> Result<Principal<User>, AuthError> {
        // `AUTH <password>` authenticates the default user; `AUTH <user> <pass>`
        // names one. Thunder parses the frame; the credential store stays ours
        // (SRV-012).
        let (user, pass) = match creds {
            Credentials::UserPass(u, p) => (u, p),
            Credentials::ApiKey(p) | Credentials::Token(p) => ("default".to_string(), p),
            Credentials::None => return Err(AuthError::InvalidCredentials),
        };

        let Some(manager) = self.state.user_manager.as_ref() else {
            // Auth was requested against a deployment with no user store —
            // there is nothing that could validate it.
            return Err(AuthError::InvalidCredentials);
        };

        // The only credential-store lookup per connection; the record it
        // returns rides the session from here on.
        manager
            .authenticate(&user, &pass)
            .map(|u| Principal::with_identity(u.username.clone(), u))
            .map_err(|_| AuthError::InvalidCredentials)
    }
}

impl SynapDispatch {
    /// Register this connection's push channel with the pubsub router, so
    /// published messages reach it as Thunder push frames (`id == PUSH_ID`).
    fn bridge_subscription(&self, session: &Session<User>, response: &SynapValue) {
        let Some(subscriber_id) = response
            .map_get("subscriber_id")
            .and_then(SynapValue::as_str)
            .map(str::to_owned)
        else {
            return;
        };
        let (Some(router), Some(push)) = (self.state.pubsub_router.as_ref(), session.push_sender())
        else {
            return;
        };

        // KV.WATCH in `notify` mode asked for envelopes without the inline
        // value; the strip happens here, per subscription, before the push.
        let notify_only = response.map_get("mode").and_then(SynapValue::as_str) == Some("notify");

        let (tx, mut rx) = tokio::sync::mpsc::channel::<PubSubMessage>(
            crate::core::pubsub::SUBSCRIBER_CHANNEL_CAPACITY,
        );
        router.register_connection(subscriber_id, tx);

        let push = push.clone();
        tokio::spawn(async move {
            while let Some(mut msg) = rx.recv().await {
                if notify_only {
                    strip_watch_value(&mut msg.payload);
                }
                let frame = SynapValue::Map(vec![
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
                if push.push(frame).await.is_err() {
                    break; // connection closed
                }
            }
        });
    }
}

/// Reduce a watch envelope to notify-only: drop the inline value, and the
/// `truncated` flag with it — a client that never asked for values has no cap
/// to be told about.
fn strip_watch_value(payload: &mut serde_json::Value) {
    if let Some(map) = payload.as_object_mut() {
        map.remove("value");
        map.remove("truncated");
    }
}

/// Spawn the SynapRPC TCP listener on `addr`.
///
/// The accept loop runs as a background task. The returned handle must be kept
/// alive for the listener's lifetime; [`ListenerHandle::stop`] drains it
/// gracefully, and dropping it shuts down without waiting.
pub async fn spawn_synap_rpc_listener(
    state: AppState,
    addr: SocketAddr,
    idle_timeout: Duration,
    max_connections: usize,
) -> std::io::Result<Arc<ListenerHandle>> {
    let require_auth = state.require_auth;
    let dispatch = Arc::new(SynapDispatch {
        state: Arc::new(state),
    });

    let mut listener_config = ListenerConfig::new(addr)
        .with_max_connections(max_connections)
        .with_observer(Arc::new(SynapMetrics));
    listener_config.idle_timeout = idle_timeout;
    listener_config.slow_threshold = SLOW_COMMAND_THRESHOLD;
    if !require_auth {
        listener_config = listener_config.open();
    }

    let info = ServerInfo {
        name: "synap".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let handle = spawn_listener(dispatch, synap_config(), info, listener_config).await?;
    tracing::info!("SynapRPC server listening on {}", handle.local_addr());

    Ok(Arc::new(handle))
}

#[cfg(test)]
mod tests {
    use super::strip_watch_value;

    #[test]
    fn notify_mode_strips_value_and_truncated() {
        let mut payload = serde_json::json!({
            "key": "user:1",
            "event": "set",
            "version": 3,
            "value": "alice",
            "truncated": false,
        });

        strip_watch_value(&mut payload);

        assert_eq!(
            payload,
            serde_json::json!({ "key": "user:1", "event": "set", "version": 3 }),
            "a notify-mode watcher gets the envelope without the value"
        );
    }
}
