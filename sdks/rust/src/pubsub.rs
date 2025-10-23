//! Pub/Sub operations

use crate::client::SynapClient;
use crate::error::{Result, SynapError};
use serde_json::json;

/// Pub/Sub Manager interface
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
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, SynapConfig};
    /// # use serde_json::json;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// client.pubsub().publish(
    ///     "notifications.email",
    ///     json!({"to": "user@example.com", "subject": "Hello"}),
    ///     None,
    ///     None
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn publish(
        &self,
        topic: &str,
        message: serde_json::Value,
        priority: Option<u8>,
        headers: Option<std::collections::HashMap<String, String>>,
    ) -> Result<usize> {
        let body = json!({
            "topic": topic,
            "message": message,
            "priority": priority,
            "headers": headers,
        });

        let response = self.client.post("pubsub/publish", body).await?;

        Ok(response["delivered_count"].as_u64().unwrap_or(0) as usize)
    }

    /// Subscribe to topics
    ///
    /// Returns a subscription ID that can be used to unsubscribe.
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, SynapConfig};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// let sub_id = client.pubsub().subscribe(vec![
    ///     "events.user.*".to_string(),
    ///     "notifications.#".to_string(),
    /// ]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn subscribe(&self, topics: Vec<String>) -> Result<String> {
        let body = json!({"topics": topics});
        let response = self.client.post("pubsub/subscribe", body).await?;

        Ok(response["subscription_id"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    /// Unsubscribe from topics
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        let body = json!({"subscription_id": subscription_id});
        let response = self.client.post("pubsub/unsubscribe", body).await?;

        if response["success"].as_bool() != Some(true) {
            return Err(SynapError::ServerError("Failed to unsubscribe".to_string()));
        }

        Ok(())
    }

    /// List all active topics
    pub async fn list_topics(&self) -> Result<Vec<String>> {
        let response = self.client.get("pubsub/topics").await?;
        Ok(serde_json::from_value(response["topics"].clone())?)
    }
}
