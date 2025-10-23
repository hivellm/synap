//! Event Stream operations

use crate::client::SynapClient;
use crate::error::Result;
use crate::types::{Event, StreamStats};
use serde_json::{Value, json};

/// Stream Manager interface
///
/// Uses StreamableHTTP protocol for all operations.
/// Event Streams are **reactive by default** - use `observe_events()` or `observe_event()`.
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
        let mut payload = json!({"room": room});
        if let Some(max) = max_events {
            payload["max_events"] = json!(max);
        }

        self.client.send_command("stream.create", payload).await?;
        Ok(())
    }

    /// Publish an event to a stream
    ///
    /// # Returns
    /// Returns the offset of the published event
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, SynapConfig};
    /// # use serde_json::json;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// let offset = client.stream().publish(
    ///     "chat-room",
    ///     "message",
    ///     json!({"user": "alice", "text": "Hello!"})
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn publish(&self, room: &str, event: &str, data: Value) -> Result<u64> {
        let payload = json!({
            "room": room,
            "event": event,
            "data": data,
        });

        let response = self.client.send_command("stream.publish", payload).await?;

        Ok(response["offset"].as_u64().unwrap_or(0))
    }

    /// Consume events from a stream
    ///
    /// # Arguments
    /// * `room` - Stream room name
    /// * `offset` - Starting offset (None = from beginning)
    /// * `limit` - Maximum events to fetch
    pub async fn consume(
        &self,
        room: &str,
        offset: Option<u64>,
        limit: Option<usize>,
    ) -> Result<Vec<Event>> {
        let payload = json!({
            "room": room,
            "offset": offset,
            "limit": limit,
        });

        let response = self.client.send_command("stream.consume", payload).await?;
        Ok(serde_json::from_value(response["events"].clone())?)
    }

    /// Get stream statistics
    pub async fn stats(&self, room: &str) -> Result<StreamStats> {
        let payload = json!({"room": room});
        let response = self.client.send_command("stream.stats", payload).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// List all stream rooms
    pub async fn list(&self) -> Result<Vec<String>> {
        let response = self.client.send_command("stream.list", json!({})).await?;
        Ok(serde_json::from_value(response["rooms"].clone())?)
    }

    /// Delete a stream room
    pub async fn delete_room(&self, room: &str) -> Result<()> {
        let payload = json!({"room": room});
        self.client.send_command("stream.delete", payload).await?;
        Ok(())
    }
}
