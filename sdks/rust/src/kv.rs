//! Key-Value Store operations

use crate::client::SynapClient;
use crate::error::Result;
use crate::types::KVStats;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Key-Value Store interface
///
/// Uses StreamableHTTP protocol for all operations.
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
    /// * `value` - The value to store
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
        let payload = json!({
            "key": key.as_ref(),
            "value": value,
            "ttl": ttl,
        });

        self.client.send_command("kv.set", payload).await?;
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
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("kv.get", payload).await?;

        // StreamableHTTP returns null for not found
        if response.is_null() {
            return Ok(None);
        }

        // Parse the value
        let value: V = serde_json::from_value(response)?;
        Ok(Some(value))
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
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("kv.del", payload).await?;

        Ok(response["deleted"].as_bool().unwrap_or(false))
    }

    /// Check if a key exists
    pub async fn exists<K>(&self, key: K) -> Result<bool>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("kv.exists", payload).await?;

        Ok(response["exists"].as_bool().unwrap_or(false))
    }

    /// Increment a numeric value
    pub async fn incr<K>(&self, key: K) -> Result<i64>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("kv.incr", payload).await?;

        Ok(response["value"].as_i64().unwrap_or(0))
    }

    /// Decrement a numeric value
    pub async fn decr<K>(&self, key: K) -> Result<i64>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("kv.decr", payload).await?;

        Ok(response["value"].as_i64().unwrap_or(0))
    }

    /// Get KV store statistics
    pub async fn stats(&self) -> Result<KVStats> {
        let response = self.client.send_command("kv.stats", json!({})).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Get all keys matching a prefix
    pub async fn keys<P>(&self, prefix: P) -> Result<Vec<String>>
    where
        P: AsRef<str>,
    {
        let payload = json!({"prefix": prefix.as_ref()});
        let response = self.client.send_command("kv.keys", payload).await?;

        Ok(serde_json::from_value(response["keys"].clone())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SynapConfig;

    #[tokio::test]
    async fn test_kv_operations() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let kv = client.kv();

        // Test that KV store can be created
        assert!(std::mem::size_of_val(&kv) > 0);
    }

    #[test]
    fn test_kv_clone() {
        let config = SynapConfig::new("http://localhost:15500");
        let client = SynapClient::new(config).unwrap();
        let kv1 = client.kv();
        let kv2 = kv1.clone();

        // Both should exist
        assert!(std::mem::size_of_val(&kv1) > 0);
        assert!(std::mem::size_of_val(&kv2) > 0);
    }
}
