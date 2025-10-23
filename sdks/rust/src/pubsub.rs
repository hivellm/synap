//! Pub/Sub operations

use crate::client::SynapClient;
use crate::error::Result;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Pub/Sub Manager interface
///
/// Uses StreamableHTTP protocol for all operations.
/// Pub/Sub is **reactive by default** - use `subscribe()` and `subscribe_topic()`.
#[derive(Clone)]
pub struct PubSubManager {
    client: SynapClient,
}

impl PubSubManager {
    /// Create a new Pub/Sub manager interface
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Publish a message to a topic
    ///
    /// # Returns
    /// Returns the number of subscribers that received the message
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, SynapConfig};
    /// # use serde_json::json;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// let count = client.pubsub().publish(
    ///     "user.created",
    ///     json!({"id": 123, "name": "Alice"}),
    ///     Some(5),
    ///     None
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn publish(
        &self,
        topic: &str,
        data: Value,
        priority: Option<u8>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<usize> {
        let payload = json!({
            "topic": topic,
            "data": data,
            "priority": priority,
            "headers": headers,
        });

        let response = self.client.send_command("pubsub.publish", payload).await?;

        Ok(response["delivered_count"].as_u64().unwrap_or(0) as usize)
    }

    /// Subscribe to topics
    ///
    /// Supports wildcard patterns:
    /// - `user.*` - single-level wildcard
    /// - `user.#` - multi-level wildcard
    ///
    /// # Returns
    /// Returns a subscription ID
    pub async fn subscribe_topics(
        &self,
        subscriber_id: &str,
        topics: Vec<String>,
    ) -> Result<String> {
        let payload = json!({
            "subscriber_id": subscriber_id,
            "topics": topics,
        });

        let response = self
            .client
            .send_command("pubsub.subscribe", payload)
            .await?;

        Ok(response["subscription_id"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    /// Unsubscribe from topics
    pub async fn unsubscribe(&self, subscriber_id: &str, topics: Vec<String>) -> Result<()> {
        let payload = json!({
            "subscriber_id": subscriber_id,
            "topics": topics,
        });

        self.client
            .send_command("pubsub.unsubscribe", payload)
            .await?;
        Ok(())
    }

    /// List all active topics
    pub async fn list_topics(&self) -> Result<Vec<String>> {
        let response = self.client.send_command("pubsub.topics", json!({})).await?;
        Ok(serde_json::from_value(response["topics"].clone())?)
    }
}
