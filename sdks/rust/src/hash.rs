//! Hash data structure operations
use crate::client::SynapClient;
use crate::error::Result;
use serde_json::json;
use std::collections::HashMap;

/// Hash data structure interface (Redis-compatible)
///
/// Hash is a field-value map, ideal for storing objects.
#[derive(Clone)]
pub struct HashManager {
    client: SynapClient,
}

impl HashManager {
    /// Create a new Hash manager interface
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Set field in hash
    pub async fn set<K, F, V>(&self, key: K, field: F, value: V) -> Result<bool>
    where
        K: AsRef<str>,
        F: AsRef<str>,
        V: ToString,
    {
        let payload = json!({
            "key": key.as_ref(),
            "field": field.as_ref(),
            "value": value.to_string(),
        });

        let response = self.client.send_command("hash.set", payload).await?;
        Ok(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Get field from hash
    pub async fn get<K, F>(&self, key: K, field: F) -> Result<Option<String>>
    where
        K: AsRef<str>,
        F: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "field": field.as_ref(),
        });

        let response = self.client.send_command("hash.get", payload).await?;
        Ok(response
            .get("value")
            .and_then(|v| v.as_str())
            .map(String::from))
    }

    /// Get all fields and values from hash
    pub async fn get_all<K>(&self, key: K) -> Result<HashMap<String, String>>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("hash.getall", payload).await?;

        let fields = response
            .get("fields")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(fields)
    }

    /// Delete field from hash
    pub async fn del<K, F>(&self, key: K, field: F) -> Result<i64>
    where
        K: AsRef<str>,
        F: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "field": field.as_ref(),
        });

        let response = self.client.send_command("hash.del", payload).await?;
        Ok(response
            .get("deleted")
            .and_then(|v| v.as_i64())
            .unwrap_or(0))
    }

    /// Check if field exists in hash
    pub async fn exists<K, F>(&self, key: K, field: F) -> Result<bool>
    where
        K: AsRef<str>,
        F: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "field": field.as_ref(),
        });

        let response = self.client.send_command("hash.exists", payload).await?;
        Ok(response
            .get("exists")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Get all field names in hash
    pub async fn keys<K>(&self, key: K) -> Result<Vec<String>>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("hash.keys", payload).await?;

        let keys = response
            .get("fields")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(keys)
    }

    /// Get all values in hash
    pub async fn values<K>(&self, key: K) -> Result<Vec<String>>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("hash.values", payload).await?;

        let values = response
            .get("values")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(values)
    }

    /// Get number of fields in hash
    pub async fn len<K>(&self, key: K) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("hash.len", payload).await?;
        Ok(response.get("length").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Set multiple fields in hash
    pub async fn mset<K>(&self, key: K, fields: HashMap<String, String>) -> Result<bool>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "fields": fields,
        });

        let response = self.client.send_command("hash.mset", payload).await?;
        Ok(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Get multiple fields from hash
    pub async fn mget<K>(
        &self,
        key: K,
        fields: Vec<String>,
    ) -> Result<HashMap<String, Option<String>>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "fields": fields,
        });

        let response = self.client.send_command("hash.mget", payload).await?;

        let values = response
            .get("values")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(values)
    }

    /// Increment field value by integer
    pub async fn incr_by<K, F>(&self, key: K, field: F, increment: i64) -> Result<i64>
    where
        K: AsRef<str>,
        F: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "field": field.as_ref(),
            "increment": increment,
        });

        let response = self.client.send_command("hash.incrby", payload).await?;
        Ok(response.get("value").and_then(|v| v.as_i64()).unwrap_or(0))
    }

    /// Increment field value by float
    pub async fn incr_by_float<K, F>(&self, key: K, field: F, increment: f64) -> Result<f64>
    where
        K: AsRef<str>,
        F: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "field": field.as_ref(),
            "increment": increment,
        });

        let response = self
            .client
            .send_command("hash.incrbyfloat", payload)
            .await?;
        Ok(response
            .get("value")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0))
    }

    /// Set field only if it doesn't exist
    pub async fn set_nx<K, F, V>(&self, key: K, field: F, value: V) -> Result<bool>
    where
        K: AsRef<str>,
        F: AsRef<str>,
        V: ToString,
    {
        let payload = json!({
            "key": key.as_ref(),
            "field": field.as_ref(),
            "value": value.to_string(),
        });

        let response = self.client.send_command("hash.setnx", payload).await?;
        Ok(response
            .get("created")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }
}
