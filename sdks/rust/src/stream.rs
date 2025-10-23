//! Event Stream operations

use crate::client::SynapClient;
use crate::error::{Result, SynapError};
use crate::types::{StreamEvent, StreamStats};
use serde_json::json;

/// Stream Manager interface
#[derive(Clone)]
pub struct StreamManager {
    pub(crate) client: SynapClient,
}

impl StreamManager {
    /// Create a new Stream manager interface
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Create a new stream room
    pub async fn create_room(&self, room: &str, max_events: Option<usize>) -> Result<()> {
        let body = json!({
            "room": room,
            "max_events": max_events,
        });

        let response = self.client.post("stream/create", body).await?;

        if response["success"].as_bool() != Some(true) {
            return Err(SynapError::ServerError("Failed to create room".to_string()));
        }

        Ok(())
    }

    /// Publish an event to a stream room
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, SynapConfig};
    /// # use serde_json::json;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// let offset = client.stream().publish(
    ///     "chat-room-1",
    ///     "message",
    ///     json!({"user": "alice", "text": "Hello!"})
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn publish(&self, room: &str, event: &str, data: serde_json::Value) -> Result<u64> {
        let body = json!({
            "room": room,
            "event": event,
            "data": data,
        });

        let response = self.client.post("stream/publish", body).await?;

        Ok(response["offset"].as_u64().unwrap_or(0))
    }

    /// Consume events from a stream room
    ///
    /// # Arguments
    /// * `room` - Room name
    /// * `offset` - Starting offset (None = from beginning)
    /// * `limit` - Maximum number of events to retrieve
    pub async fn consume(
        &self,
        room: &str,
        offset: Option<u64>,
        limit: Option<usize>,
    ) -> Result<Vec<StreamEvent>> {
        let body = json!({
            "room": room,
            "offset": offset,
            "limit": limit,
        });

        let response = self.client.post("stream/consume", body).await?;

        Ok(serde_json::from_value(response["events"].clone())?)
    }

    /// Get stream room statistics
    pub async fn stats(&self, room: &str) -> Result<StreamStats> {
        let body = json!({"room": room});
        let response = self.client.post("stream/stats", body).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// List all stream rooms
    pub async fn list(&self) -> Result<Vec<String>> {
        let response = self.client.post("stream/list", json!({})).await?;
        Ok(serde_json::from_value(response["rooms"].clone())?)
    }

    /// Delete a stream room
    pub async fn delete_room(&self, room: &str) -> Result<()> {
        let body = json!({"room": room});
        let response = self.client.post("stream/delete", body).await?;

        if response.as_str() != Some(room) && response["success"].as_bool() != Some(true) {
            return Err(SynapError::ServerError("Failed to delete room".to_string()));
        }

        Ok(())
    }
}
