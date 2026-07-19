//! Reactive KV watch — observe a key (or pattern) and receive its new value.
//!
//! Rides the same infrastructure as [`crate::pubsub_reactive`]: the `synap://`
//! transport opens a dedicated push connection driven by `KV.WATCH`, and
//! `http://` / `https://` URLs fall back to the `/kv/ws` WebSocket endpoint.
//! See `docs/kv-watch.md` in the server repository for the envelope semantics.

use crate::reactive::{MessageStream, SubscriptionHandle};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

/// One key-change event delivered to a watcher.
///
/// `value` is the **post-mutation** value and is absent for terminal events
/// (`del`, `expired`, `evicted`), TTL-only events (`expire`, `persist`), and
/// envelopes degraded to notify-only (`truncated: true` — oversized or
/// non-UTF-8 value; re-`GET` if the payload matters).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatchEvent {
    /// The key that changed.
    pub key: String,
    /// What happened: `set`, `del`, `expired`, `evicted`, `expire`, `persist`,
    /// `append`, `setrange`, `incrby`, `decrby`.
    pub event: String,
    /// Per-key counter for gap detection. Resets when the key is deleted,
    /// expires or is evicted — version 1 marks a new incarnation.
    pub version: u64,
    /// The post-mutation value, when inlined.
    #[serde(default)]
    pub value: Option<String>,
    /// `true` when the value was withheld (over the inline cap, or not UTF-8).
    #[serde(default)]
    pub truncated: bool,
}

/// Per-subscription delivery mode for [`KVStore::watch_with_mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WatchMode {
    /// Envelopes carry the post-mutation value (up to the server's inline cap).
    #[default]
    Value,
    /// Envelopes carry key/event/version only; the server strips the value, so
    /// a watcher that only wants change signals pays no value bandwidth.
    Notify,
}

impl crate::kv::KVStore {
    /// Watch a key (or wildcard pattern) and stream its change events.
    ///
    /// Delivery is best-effort, latest-value: a watcher that cannot keep up is
    /// disconnected by the server and must re-`GET` and re-watch. Use
    /// [`WatchEvent::version`] to detect gaps.
    ///
    /// # Example
    /// ```no_run
    /// use futures::StreamExt;
    /// use synap_sdk::{SynapClient, SynapConfig};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("synap://localhost:15501"))?;
    /// let (mut events, handle) = client.kv().watch("user:*");
    ///
    /// while let Some(event) = events.next().await {
    ///     println!("{} {} v{} = {:?}", event.event, event.key, event.version, event.value);
    /// }
    ///
    /// handle.unsubscribe();
    /// # Ok(())
    /// # }
    /// ```
    pub fn watch(
        &self,
        pattern: impl Into<String>,
    ) -> (impl Stream<Item = WatchEvent> + 'static, SubscriptionHandle) {
        self.watch_with_mode(pattern, WatchMode::Value)
    }

    /// [`Self::watch`] with an explicit delivery mode.
    ///
    /// [`WatchMode::Notify`] is honored on the SynapRPC transport; the
    /// WebSocket fallback always delivers value envelopes.
    pub fn watch_with_mode(
        &self,
        pattern: impl Into<String>,
        mode: WatchMode,
    ) -> (impl Stream<Item = WatchEvent> + 'static, SubscriptionHandle) {
        let pattern = pattern.into();
        let client = self.client.clone();

        let (tx, rx) = mpsc::unbounded_channel::<WatchEvent>();
        let (cancel_tx, mut cancel_rx) = mpsc::unbounded_channel::<()>();

        tokio::spawn(async move {
            // ── SynapRPC native push path ─────────────────────────────────────
            if let Some(rpc) = client.synap_rpc_transport() {
                let mode_arg = match mode {
                    WatchMode::Value => None,
                    WatchMode::Notify => Some("notify".to_string()),
                };
                match rpc.watch_push(pattern, mode_arg).await {
                    // `subscription` owns the connection: holding it for the
                    // life of this loop is what keeps the push socket open.
                    Ok(mut subscription) => {
                        tracing::debug!(
                            sub_id = %subscription.subscriber_id,
                            "KV watch push connection established"
                        );
                        loop {
                            tokio::select! {
                                _ = cancel_rx.recv() => {
                                    // Teardown issues KV.UNWATCH on the same
                                    // connection before it closes.
                                    subscription.unwatch().await;
                                    tracing::debug!("KV watch stream cancelled");
                                    break;
                                }
                                msg = subscription.messages.recv() => {
                                    match msg {
                                        Some(json) => {
                                            if let Some(event) = decode_push_frame(&json)
                                                && tx.send(event).is_err()
                                            {
                                                break; // downstream receiver dropped
                                            }
                                        }
                                        None => {
                                            tracing::debug!("KV watch push connection closed");
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("SynapRPC KV.WATCH failed: {}", e);
                    }
                }
                return;
            }

            // ── WebSocket fallback (HTTP / HTTPS transport) ───────────────────
            if mode == WatchMode::Notify {
                tracing::warn!(
                    "WatchMode::Notify is RPC-only; the WebSocket fallback delivers value envelopes"
                );
            }
            let base_url = client.base_url();
            let ws_url = match base_url.scheme() {
                "http" => format!("ws://{}", base_url.authority()),
                "https" => format!("wss://{}", base_url.authority()),
                _ => {
                    tracing::error!(
                        "Unsupported URL scheme for WebSocket KV watch: {}",
                        base_url.scheme()
                    );
                    return;
                }
            };
            let ws_endpoint = format!("{}/kv/ws?keys={}", ws_url, pattern);

            let ws_stream = match connect_async(&ws_endpoint).await {
                Ok((stream, _)) => stream,
                Err(e) => {
                    tracing::error!("Failed to connect KV watch WebSocket: {}", e);
                    return;
                }
            };
            let (_write, mut read) = ws_stream.split();

            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => {
                        tracing::debug!("KV watch stream cancelled");
                        break;
                    }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(WsMessage::Text(text))) => {
                                if let Some(event) = decode_ws_frame(&text)
                                    && tx.send(event).is_err()
                                {
                                    break;
                                }
                            }
                            Some(Ok(WsMessage::Close(_))) => {
                                tracing::debug!("KV watch WebSocket closed by server");
                                break;
                            }
                            Some(Ok(_)) => {}
                            Some(Err(e)) => {
                                tracing::error!("KV watch WebSocket error: {}", e);
                                break;
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        let stream: MessageStream<WatchEvent> =
            Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx));
        let handle = SubscriptionHandle::new(cancel_tx);

        (stream, handle)
    }
}

/// Decode one SynapRPC push frame — `{ topic, payload (JSON string), id,
/// timestamp }` — into a watch envelope. Non-watch frames decode to `None`.
fn decode_push_frame(frame: &serde_json::Value) -> Option<WatchEvent> {
    let payload = frame.get("payload")?;
    // The bridge stringifies the envelope; tolerate an already-decoded object.
    match payload.as_str() {
        Some(s) => serde_json::from_str(s).ok(),
        None => serde_json::from_value(payload.clone()).ok(),
    }
}

/// Decode one `/kv/ws` frame — `{"type": "message", "payload": <envelope>}` —
/// into a watch envelope. Welcome and error frames decode to `None`.
fn decode_ws_frame(text: &str) -> Option<WatchEvent> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    if json.get("type").and_then(|t| t.as_str()) != Some("message") {
        return None;
    }
    serde_json::from_value(json.get("payload")?.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_envelope_decodes() {
        let frame = serde_json::json!({
            "topic": "__watch@0__:user:1",
            "payload": r#"{"key":"user:1","event":"set","version":3,"value":"alice"}"#,
            "id": "m1",
            "timestamp": 1,
        });

        let event = decode_push_frame(&frame).expect("decodes");
        assert_eq!(
            event,
            WatchEvent {
                key: "user:1".to_string(),
                event: "set".to_string(),
                version: 3,
                value: Some("alice".to_string()),
                truncated: false,
            }
        );
    }

    #[test]
    fn omitted_optional_fields_take_defaults() {
        // A del envelope omits value and truncated entirely.
        let frame = serde_json::json!({
            "topic": "__watch@0__:k",
            "payload": r#"{"key":"k","event":"del","version":7}"#,
        });

        let event = decode_push_frame(&frame).expect("decodes");
        assert_eq!(event.value, None);
        assert!(!event.truncated);
        assert_eq!(event.version, 7);
    }

    #[test]
    fn a_truncated_envelope_keeps_the_flag() {
        let frame = serde_json::json!({
            "topic": "__watch@0__:big",
            "payload": r#"{"key":"big","event":"set","version":1,"truncated":true}"#,
        });

        let event = decode_push_frame(&frame).expect("decodes");
        assert!(event.truncated);
        assert_eq!(event.value, None);
    }

    #[test]
    fn ws_welcome_frames_are_skipped() {
        assert!(
            decode_ws_frame(r#"{"type":"connected","subscriber_id":"s1"}"#).is_none(),
            "only message frames become events"
        );

        let event = decode_ws_frame(
            r#"{"type":"message","topic":"__watch@0__:k","payload":{"key":"k","event":"set","version":1,"value":"v"}}"#,
        )
        .expect("message frames decode");
        assert_eq!(event.value.as_deref(), Some("v"));
    }

    #[tokio::test]
    async fn watch_compiles_and_tears_down() {
        let config = crate::SynapConfig::new("http://localhost:15500");
        let client = crate::SynapClient::new(config).unwrap();

        let (_stream, handle) = client.kv().watch("test:key");

        assert!(handle.is_active());
        handle.unsubscribe();
    }
}
