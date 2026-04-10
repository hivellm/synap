//! Event Stream operations

use crate::client::SynapClient;
use crate::error::Result;
use crate::types::{Event, StreamStats};
use serde::Deserialize;
use serde_json::{Value, json};

/// Wire format of a stream event as returned by the server.
/// HTTP returns `data` as `Vec<u8>` (serde_json::to_vec of the original JSON).
/// SynapRPC may return `data` as a string.  We accept both via `Value`.
#[derive(Deserialize)]
struct RawStreamEvent {
    #[serde(default)]
    offset: u64,
    event: String,
    data: Value,
    #[serde(default)]
    timestamp: Option<u64>,
}

impl From<RawStreamEvent> for Event {
    fn from(raw: RawStreamEvent) -> Self {
        let data = match &raw.data {
            // HTTP: data arrives as JSON array of bytes → decode back to JSON
            Value::Array(arr) => {
                let bytes: Vec<u8> = arr
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect();
                serde_json::from_slice(&bytes).unwrap_or(Value::Null)
            }
            // RPC: data arrives as a JSON string (the serialized JSON)
            Value::String(s) => serde_json::from_str(s).unwrap_or(Value::String(s.clone())),
            // Already a JSON value (unlikely but handle gracefully)
            other => other.clone(),
        };
        Self {
            offset: raw.offset,
            event: raw.event,
            data,
            timestamp: raw.timestamp,
        }
    }
}

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
            "subscriber_id": "sdk-default",
            "from_offset": offset.unwrap_or(0),
            "limit": limit,
        });

        let response = self.client.send_command("stream.consume", payload).await?;

        // Server stores data as Vec<u8> (serde_json::to_vec) and serialises the
        // struct directly, so `data` arrives as a JSON byte-array.  We decode
        // each event's data bytes back into the original JSON value.
        let raw_events: Vec<RawStreamEvent> = serde_json::from_value(response["events"].clone())?;
        Ok(raw_events.into_iter().map(Into::into).collect())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SynapConfig;

    #[test]
    fn test_stream_manager_creation() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let stream = client.stream();

        assert!(std::mem::size_of_val(&stream) > 0);
    }

    #[test]
    fn test_stream_manager_clone() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let stream1 = client.stream();
        let stream2 = stream1.clone();

        assert!(std::mem::size_of_val(&stream1) > 0);
        assert!(std::mem::size_of_val(&stream2) > 0);
    }
}
