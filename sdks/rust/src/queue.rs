//! Queue operations

use crate::client::SynapClient;
use crate::error::{Result, SynapError};
use crate::types::{Message, QueueStats};
use serde_json::json;

// Re-export for convenience
pub use crate::reactive::{MessageStream, SubscriptionHandle};

/// Queue Manager interface
#[derive(Clone)]
pub struct QueueManager {
    pub(crate) client: SynapClient,
}

impl QueueManager {
    /// Create a new Queue manager interface
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Create a new queue
    ///
    /// # Arguments
    /// * `queue_name` - Name of the queue
    /// * `max_depth` - Maximum queue depth (optional)
    /// * `ack_deadline_secs` - ACK deadline in seconds (optional)
    pub async fn create_queue(
        &self,
        queue_name: &str,
        max_depth: Option<usize>,
        ack_deadline_secs: Option<u64>,
    ) -> Result<()> {
        let mut body = json!({});
        if let Some(depth) = max_depth {
            body["max_depth"] = json!(depth);
        }
        if let Some(deadline) = ack_deadline_secs {
            body["ack_deadline_secs"] = json!(deadline);
        }

        let path = format!("queue/{}", queue_name);
        let response = self.client.post(&path, body).await?;

        if response["success"].as_bool() != Some(true) {
            return Err(SynapError::ServerError(
                "Failed to create queue".to_string(),
            ));
        }

        Ok(())
    }

    /// Publish a message to a queue
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, SynapConfig};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// client.queue().publish("tasks", b"process-video", Some(9), None).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn publish(
        &self,
        queue_name: &str,
        payload: &[u8],
        priority: Option<u8>,
        max_retries: Option<u32>,
    ) -> Result<String> {
        let path = format!("queue/{}/publish", queue_name);
        let body = json!({
            "payload": payload,
            "priority": priority,
            "max_retries": max_retries,
        });

        let response = self.client.post(&path, body).await?;

        Ok(response["message_id"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    /// Consume a message from a queue
    pub async fn consume(&self, queue_name: &str, consumer_id: &str) -> Result<Option<Message>> {
        let path = format!("queue/{}/consume/{}", queue_name, consumer_id);
        let response = self.client.get(&path).await?;

        if response.is_null() {
            return Ok(None);
        }

        Ok(serde_json::from_value(response).ok())
    }

    /// Acknowledge a message
    pub async fn ack(&self, queue_name: &str, message_id: &str) -> Result<()> {
        let path = format!("queue/{}/ack", queue_name);
        let body = json!({"message_id": message_id});

        let response = self.client.post(&path, body).await?;

        if response["success"].as_bool() != Some(true) {
            return Err(SynapError::ServerError("Failed to ACK message".to_string()));
        }

        Ok(())
    }

    /// Negative acknowledge a message (requeue)
    pub async fn nack(&self, queue_name: &str, message_id: &str) -> Result<()> {
        let path = format!("queue/{}/nack", queue_name);
        let body = json!({"message_id": message_id});

        let response = self.client.post(&path, body).await?;

        if response["success"].as_bool() != Some(true) {
            return Err(SynapError::ServerError(
                "Failed to NACK message".to_string(),
            ));
        }

        Ok(())
    }

    /// Get queue statistics
    pub async fn stats(&self, queue_name: &str) -> Result<QueueStats> {
        let path = format!("queue/{}/stats", queue_name);
        let response = self.client.get(&path).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// List all queues
    pub async fn list(&self) -> Result<Vec<String>> {
        let response = self.client.get("queue/list").await?;
        Ok(serde_json::from_value(response["queues"].clone())?)
    }

    /// Delete a queue
    pub async fn delete_queue(&self, queue_name: &str) -> Result<()> {
        let path = format!("queue/{}", queue_name);
        let response = self.client.delete(&path).await?;

        if response["success"].as_bool() != Some(true) {
            return Err(SynapError::ServerError(
                "Failed to delete queue".to_string(),
            ));
        }

        Ok(())
    }
}
