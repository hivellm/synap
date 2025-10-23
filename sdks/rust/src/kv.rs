//! Key-Value Store operations

use crate::client::SynapClient;
use crate::error::{Result, SynapError};
use crate::types::KVStats;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Key-Value Store interface
#[derive(Clone)]
pub struct KVStore {
    client: SynapClient,
}

impl KVStore {
    /// Create a new KV store interface
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Set a key-value pair
    ///
    /// # Arguments
    /// * `key` - The key to set
    /// * `value` - The value to set (must be serializable to JSON)
    /// * `ttl` - Optional time-to-live in seconds
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, SynapConfig};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// client.kv().set("user:1", "John Doe", None).await?;
    /// client.kv().set("session:abc", "token123", Some(3600)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set<K, V>(&self, key: K, value: V, ttl: Option<u64>) -> Result<()>
    where
        K: AsRef<str>,
        V: Serialize,
    {
        let body = json!({
            "key": key.as_ref(),
            "value": value,
            "ttl": ttl,
        });

        let response = self.client.post("kv/set", body).await?;

        if response["success"].as_bool() != Some(true) {
            return Err(SynapError::ServerError("Failed to set key".to_string()));
        }

        Ok(())
    }

    /// Get a value by key
    ///
    /// Returns `None` if the key doesn't exist or has expired.
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, SynapConfig};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// let value: Option<String> = client.kv().get("user:1").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<str>,
        V: for<'de> Deserialize<'de>,
    {
        let path = format!("kv/get/{}", key.as_ref());
        let response = self.client.get(&path).await?;

        // Check if it's an error response
        if response.get("error").is_some() {
            return Ok(None);
        }

        // Server returns value directly as JSON string
        if let Some(value_str) = response.as_str() {
            // Double-encoded: parse the string to get the actual value
            let value: V = serde_json::from_str(value_str)?;
            return Ok(Some(value));
        }

        Ok(None)
    }

    /// Delete a key
    ///
    /// # Example
    /// ```no_run
    /// # use synap_sdk::{SynapClient, SynapConfig};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = SynapClient::new(SynapConfig::new("http://localhost:15500"))?;
    /// client.kv().delete("user:1").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete<K>(&self, key: K) -> Result<bool>
    where
        K: AsRef<str>,
    {
        let path = format!("kv/del/{}", key.as_ref());
        let response = self.client.delete(&path).await?;

        Ok(response["deleted"].as_bool().unwrap_or(false))
    }

    /// Check if a key exists
    pub async fn exists<K>(&self, key: K) -> Result<bool>
    where
        K: AsRef<str>,
    {
        let body = json!({"key": key.as_ref()});
        let response = self
            .client
            .post(
                "api/v1/command",
                json!({
                    "command": "kv.exists",
                    "request_id": uuid::Uuid::new_v4().to_string(),
                    "payload": body,
                }),
            )
            .await?;

        Ok(response["payload"]["exists"].as_bool().unwrap_or(false))
    }

    /// Increment a numeric value
    pub async fn incr<K>(&self, key: K) -> Result<i64>
    where
        K: AsRef<str>,
    {
        let body = json!({"key": key.as_ref()});
        let response = self.client.post("kv/incr", body).await?;

        Ok(response["value"].as_i64().unwrap_or(0))
    }

    /// Decrement a numeric value
    pub async fn decr<K>(&self, key: K) -> Result<i64>
    where
        K: AsRef<str>,
    {
        let body = json!({"key": key.as_ref()});
        let response = self.client.post("kv/decr", body).await?;

        Ok(response["value"].as_i64().unwrap_or(0))
    }

    /// Get KV store statistics
    pub async fn stats(&self) -> Result<KVStats> {
        let response = self.client.get("kv/stats").await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Get all keys matching a prefix
    pub async fn keys<P>(&self, prefix: P) -> Result<Vec<String>>
    where
        P: AsRef<str>,
    {
        let body = json!({"prefix": prefix.as_ref()});
        let response = self
            .client
            .post(
                "api/v1/command",
                json!({
                    "command": "kv.keys",
                    "request_id": uuid::Uuid::new_v4().to_string(),
                    "payload": body,
                }),
            )
            .await?;

        Ok(serde_json::from_value(response["payload"]["keys"].clone())?)
    }
}

// Add uuid to dependencies
use uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SynapConfig;

    #[tokio::test]
    async fn test_kv_operations() {
        // This is a basic compilation test
        // Real tests would require a running server or mocks
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let kv = client.kv();

        // Just verify the KVStore is created
        assert!(std::mem::size_of_val(&kv) > 0);
    }
}
