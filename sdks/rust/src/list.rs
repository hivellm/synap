//! List data structure operations

use crate::client::SynapClient;
use crate::error::Result;
use serde_json::json;

/// List data structure interface (Redis-compatible)
///
/// List is a doubly-linked list with O(1) push/pop at both ends.
#[derive(Clone)]
pub struct ListManager {
    client: SynapClient,
}

impl ListManager {
    /// Create a new List manager interface
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Push elements to left (head) of list
    pub async fn lpush<K>(&self, key: K, values: Vec<String>) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "values": values,
        });

        let response = self.client.send_command("list.lpush", payload).await?;
        Ok(response.get("length").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Push elements to right (tail) of list
    pub async fn rpush<K>(&self, key: K, values: Vec<String>) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "values": values,
        });

        let response = self.client.send_command("list.rpush", payload).await?;
        Ok(response.get("length").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Pop elements from left (head) of list
    ///
    /// # Arguments
    /// * `key` - The list key
    /// * `count` - Number of elements to pop (optional, defaults to 1)
    pub async fn lpop<K>(&self, key: K, count: Option<usize>) -> Result<Vec<String>>
    where
        K: AsRef<str>,
    {
        let mut payload = json!({
            "key": key.as_ref(),
        });

        if let Some(c) = count {
            payload["count"] = json!(c);
        }

        let response = self.client.send_command("list.lpop", payload).await?;
        let values = response
            .get("values")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(values)
    }

    /// Pop elements from right (tail) of list
    ///
    /// # Arguments
    /// * `key` - The list key
    /// * `count` - Number of elements to pop (optional, defaults to 1)
    pub async fn rpop<K>(&self, key: K, count: Option<usize>) -> Result<Vec<String>>
    where
        K: AsRef<str>,
    {
        let mut payload = json!({
            "key": key.as_ref(),
        });

        if let Some(c) = count {
            payload["count"] = json!(c);
        }

        let response = self.client.send_command("list.rpop", payload).await?;
        let values = response
            .get("values")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(values)
    }

    /// Get range of elements from list
    pub async fn range<K>(&self, key: K, start: i64, stop: i64) -> Result<Vec<String>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "start": start,
            "stop": stop,
        });

        let response = self.client.send_command("list.range", payload).await?;
        let values = response
            .get("values")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(values)
    }

    /// Get list length
    pub async fn len<K>(&self, key: K) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("list.len", payload).await?;
        Ok(response.get("length").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Get element at index
    pub async fn index<K>(&self, key: K, index: i64) -> Result<Option<String>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "index": index,
        });

        let response = self.client.send_command("list.index", payload).await?;
        Ok(response
            .get("value")
            .and_then(|v| v.as_str())
            .map(String::from))
    }

    /// Set element at index
    pub async fn set<K>(&self, key: K, index: i64, value: String) -> Result<bool>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "index": index,
            "value": value,
        });

        let response = self.client.send_command("list.set", payload).await?;
        Ok(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Trim list to specified range
    pub async fn trim<K>(&self, key: K, start: i64, stop: i64) -> Result<bool>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "start": start,
            "stop": stop,
        });

        let response = self.client.send_command("list.trim", payload).await?;
        Ok(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Remove elements from list
    pub async fn rem<K>(&self, key: K, count: i64, value: String) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "count": count,
            "value": value,
        });

        let response = self.client.send_command("list.rem", payload).await?;
        Ok(response
            .get("removed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize)
    }

    /// Insert element before/after pivot
    pub async fn insert<K>(
        &self,
        key: K,
        position: &str,
        pivot: String,
        value: String,
    ) -> Result<i64>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "position": position.to_lowercase(),
            "pivot": pivot,
            "value": value,
        });

        let response = self.client.send_command("list.insert", payload).await?;
        Ok(response
            .get("length")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1))
    }

    /// Pop from source and push to destination (atomic)
    pub async fn rpoplpush<S, D>(&self, source: S, destination: D) -> Result<Option<String>>
    where
        S: AsRef<str>,
        D: AsRef<str>,
    {
        let payload = json!({
            "source": source.as_ref(),
            "destination": destination.as_ref(),
        });

        let response = self.client.send_command("list.rpoplpush", payload).await?;
        Ok(response
            .get("value")
            .and_then(|v| v.as_str())
            .map(String::from))
    }

    /// Find first position of element
    pub async fn pos<K>(&self, key: K, element: String, rank: i64) -> Result<Option<i64>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "element": element,
            "rank": rank,
        });

        let response = self.client.send_command("list.pos", payload).await?;
        Ok(response.get("position").and_then(|v| v.as_i64()))
    }

    /// Push to left only if list exists
    pub async fn lpushx<K>(&self, key: K, values: Vec<String>) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "values": values,
        });

        let response = self.client.send_command("list.lpushx", payload).await?;
        Ok(response.get("length").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Push to right only if list exists
    pub async fn rpushx<K>(&self, key: K, values: Vec<String>) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "values": values,
        });

        let response = self.client.send_command("list.rpushx", payload).await?;
        Ok(response.get("length").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }
}
