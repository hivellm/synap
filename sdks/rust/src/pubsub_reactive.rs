//! Reactive Pub/Sub operations
//!
//! Provides Stream-based message consumption for Pub/Sub topics using WebSocket.

use crate::reactive::{MessageStream, SubscriptionHandle};
use crate::types::PubSubMessage;
use futures::{Stream, StreamExt};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

impl crate::pubsub::PubSubManager {
    /// Observe messages from Pub/Sub topics reactively using WebSocket
    ///
    /// Returns a Stream of messages that are delivered in real-time via WebSocket.
    /// Supports wildcard patterns:
    /// - `user.*` - single-level wildcard
    /// - `user.#` - multi-level wildcard
    ///
    /// # Arguments
    /// * `subscriber_id` - Unique subscriber identifier
    /// * `topics` - List of topics to subscribe to (supports wildcards)
    ///
    /// # Example
    /// ```no_run
    /// use futures::StreamExt;
    /// use synap_sdk::{SynapClient, SynapConfig};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// let (mut stream, handle) = client.pubsub()
    ///     .observe("subscriber-1", vec!["user.*".to_string(), "events.#".to_string()]);
    ///
    /// // Process messages reactively
    /// while let Some(message) = stream.next().await {
    ///     println!("Received on {}: {:?}", message.topic, message.data);
    /// }
    ///
    /// // Stop subscribing
    /// handle.unsubscribe();
    /// # Ok(())
    /// # }
    /// ```
    pub fn observe(
        &self,
        subscriber_id: impl Into<String>,
        topics: Vec<String>,
    ) -> (
        impl Stream<Item = PubSubMessage> + 'static,
        SubscriptionHandle,
    ) {
        let _subscriber_id = subscriber_id.into();
        let client = self.client.clone();
        let topics_clone = topics.clone();

        let (tx, rx) = mpsc::unbounded_channel::<PubSubMessage>();
        let (cancel_tx, mut cancel_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            // Build WebSocket URL
            let base_url = client.base_url();
            let ws_url = match base_url.scheme() {
                "http" => format!("ws://{}", base_url.authority()),
                "https" => format!("wss://{}", base_url.authority()),
                _ => {
                    tracing::error!("Unsupported URL scheme: {}", base_url.scheme());
                    return;
                }
            };

            // Build query string with topics
            let topics_query = topics_clone.join(",");
            let ws_endpoint = format!("{}/pubsub/ws?topics={}", ws_url, topics_query);

            tracing::debug!("Connecting to WebSocket: {}", ws_endpoint);

            // Connect to WebSocket
            let ws_stream = match connect_async(&ws_endpoint).await {
                Ok((stream, _)) => stream,
                Err(e) => {
                    tracing::error!("Failed to connect WebSocket: {}", e);
                    return;
                }
            };

            let (_write, mut read) = ws_stream.split();

            // Process WebSocket messages
            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => {
                        tracing::debug!("PubSub stream cancelled");
                        break;
                    }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(WsMessage::Text(text))) => {
                                // Parse JSON message
                                match serde_json::from_str::<Value>(&text) {
                                    Ok(json) => {
                                        // Handle different message types
                                        if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
                                            match msg_type {
                                                "connected" => {
                                                    tracing::debug!("PubSub WebSocket connected: {:?}", json);
                                                    // Continue to receive messages
                                                }
                                                "message" | "publish" => {
                                                    // Server sends: { "type": "message", "topic": "...", "payload": {...}, "metadata": {...} }
                                                    // SDK expects: PubSubMessage { topic, data, priority, headers }
                                                    if let (Some(topic), Some(payload)) = (
                                                        json.get("topic").and_then(|t| t.as_str()),
                                                        json.get("payload")
                                                    ) {
                                                        let pubsub_msg = PubSubMessage {
                                                            topic: topic.to_string(),
                                                            data: payload.clone(),
                                                            priority: json.get("priority").and_then(|p| p.as_u64().map(|u| u as u8)),
                                                            headers: json.get("metadata").and_then(|h| serde_json::from_value(h.clone()).ok()),
                                                        };
                                                        if tx.send(pubsub_msg).is_err() {
                                                            break; // Receiver dropped
                                                        }
                                                    }
                                                }
                                                "error" => {
                                                    if let Some(error_msg) = json.get("error").and_then(|e| e.as_str()) {
                                                        tracing::error!("PubSub WebSocket error: {}", error_msg);
                                                    }
                                                }
                                                _ => {
                                                    tracing::debug!("Unknown message type: {}", msg_type);
                                                }
                                            }
                                        } else {
                                            // No type field, assume it's a message
                                            // Try both "payload" (server format) and "data" (SDK format)
                                            if let Some(topic) = json.get("topic").and_then(|t| t.as_str()) {
                                                if let Some(payload_or_data) = json.get("payload").or_else(|| json.get("data")) {
                                                    let pubsub_msg = PubSubMessage {
                                                        topic: topic.to_string(),
                                                        data: payload_or_data.clone(),
                                                        priority: json.get("priority").and_then(|p| p.as_u64().map(|u| u as u8)),
                                                        headers: json.get("metadata")
                                                            .or_else(|| json.get("headers"))
                                                            .and_then(|h| serde_json::from_value(h.clone()).ok()),
                                                    };
                                                    if tx.send(pubsub_msg).is_err() {
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to parse WebSocket message: {}", e);
                                    }
                                }
                            }
                            Some(Ok(WsMessage::Close(_))) => {
                                tracing::debug!("WebSocket closed by server");
                                break;
                            }
                            Some(Ok(WsMessage::Ping(_data))) => {
                                // Ping received, server will handle pong automatically
                                // Continue processing
                            }
                            Some(Ok(_)) => {
                                // Ignore other message types
                            }
                            Some(Err(e)) => {
                                tracing::error!("WebSocket error: {}", e);
                                break;
                            }
                            None => {
                                tracing::debug!("WebSocket stream ended");
                                break;
                            }
                        }
                    }
                }
            }

            tracing::debug!("PubSub WebSocket connection closed");
        });

        let stream: MessageStream<PubSubMessage> =
            Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(rx));
        let handle = SubscriptionHandle::new(cancel_tx);

        (stream, handle)
    }

    /// Observe messages from a single topic reactively
    ///
    /// Convenience method for subscribing to a single topic.
    ///
    /// # Example
    /// ```no_run
    /// use futures::StreamExt;
    /// use synap_sdk::{SynapClient, SynapConfig};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// let (mut stream, handle) = client.pubsub()
    ///     .observe_topic("subscriber-1", "user.events");
    ///
    /// while let Some(message) = stream.next().await {
    ///     println!("Received: {:?}", message);
    /// }
    ///
    /// handle.unsubscribe();
    /// # Ok(())
    /// # }
    /// ```
    pub fn observe_topic(
        &self,
        subscriber_id: impl Into<String>,
        topic: impl Into<String>,
    ) -> (
        impl Stream<Item = PubSubMessage> + 'static,
        SubscriptionHandle,
    ) {
        self.observe(subscriber_id, vec![topic.into()])
    }
}

#[cfg(test)]
mod tests {
    use crate::SynapConfig;

    #[tokio::test]
    async fn test_pubsub_reactive_creation() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = crate::SynapClient::new(config).unwrap();
        let pubsub = client.pubsub();

        // Just verify the method exists and compiles
        // Note: This will spawn a tokio task but won't actually connect
        // since we're not waiting for the connection to complete
        let (_stream, _handle) = pubsub.observe("test-sub", vec!["test.topic".to_string()]);

        // Immediately unsubscribe to clean up
        _handle.unsubscribe();
    }

    #[tokio::test]
    async fn test_pubsub_reactive_single_topic() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = crate::SynapClient::new(config).unwrap();
        let pubsub = client.pubsub();

        // Just verify the method exists and compiles
        // Note: This will spawn a tokio task but won't actually connect
        // since we're not waiting for the connection to complete
        let (_stream, _handle) = pubsub.observe_topic("test-sub", "test.topic");

        // Immediately unsubscribe to clean up
        _handle.unsubscribe();
    }
}
