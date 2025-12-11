//! Set data structure operations

use crate::client::SynapClient;
use crate::error::Result;
use serde_json::json;

/// Set data structure interface (Redis-compatible)
///
/// Set is a collection of unique strings with set algebra operations.
#[derive(Clone)]
pub struct SetManager {
    client: SynapClient,
}

impl SetManager {
    /// Create a new Set manager interface
    pub(crate) fn new(client: SynapClient) -> Self {
        Self { client }
    }

    /// Add members to set
    pub async fn add<K>(&self, key: K, members: Vec<String>) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "members": members,
        });

        let response = self.client.send_command("set.add", payload).await?;
        Ok(response.get("added").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
    }

    /// Remove members from set
    pub async fn rem<K>(&self, key: K, members: Vec<String>) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "members": members,
        });

        let response = self.client.send_command("set.rem", payload).await?;
        Ok(response
            .get("removed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize)
    }

    /// Check if member exists in set
    pub async fn is_member<K>(&self, key: K, member: String) -> Result<bool>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "member": member,
        });

        let response = self.client.send_command("set.ismember", payload).await?;
        Ok(response
            .get("is_member")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Get all members of set
    pub async fn members<K>(&self, key: K) -> Result<Vec<String>>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("set.members", payload).await?;

        let members = response
            .get("members")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(members)
    }

    /// Get set cardinality (size)
    pub async fn card<K>(&self, key: K) -> Result<usize>
    where
        K: AsRef<str>,
    {
        let payload = json!({"key": key.as_ref()});
        let response = self.client.send_command("set.card", payload).await?;
        Ok(response
            .get("cardinality")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize)
    }

    /// Remove and return random members
    pub async fn pop<K>(&self, key: K, count: usize) -> Result<Vec<String>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "count": count,
        });

        let response = self.client.send_command("set.pop", payload).await?;
        let members = response
            .get("members")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(members)
    }

    /// Get random members without removing
    pub async fn rand_member<K>(&self, key: K, count: usize) -> Result<Vec<String>>
    where
        K: AsRef<str>,
    {
        let payload = json!({
            "key": key.as_ref(),
            "count": count,
        });

        let response = self.client.send_command("set.randmember", payload).await?;
        let members = response
            .get("members")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(members)
    }

    /// Move member from source to destination set
    pub async fn r#move<S, D>(&self, source: S, destination: D, member: String) -> Result<bool>
    where
        S: AsRef<str>,
        D: AsRef<str>,
    {
        let payload = json!({
            "source": source.as_ref(),
            "destination": destination.as_ref(),
            "member": member,
        });

        let response = self.client.send_command("set.move", payload).await?;
        Ok(response
            .get("moved")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Get intersection of sets
    pub async fn inter(&self, keys: Vec<String>) -> Result<Vec<String>> {
        let payload = json!({"keys": keys});
        let response = self.client.send_command("set.inter", payload).await?;

        let members = response
            .get("members")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(members)
    }

    /// Get union of sets
    pub async fn union(&self, keys: Vec<String>) -> Result<Vec<String>> {
        let payload = json!({"keys": keys});
        let response = self.client.send_command("set.union", payload).await?;

        let members = response
            .get("members")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(members)
    }

    /// Get difference of sets (first minus others)
    pub async fn diff(&self, keys: Vec<String>) -> Result<Vec<String>> {
        let payload = json!({"keys": keys});
        let response = self.client.send_command("set.diff", payload).await?;

        let members = response
            .get("members")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(members)
    }

    /// Store intersection result in destination
    pub async fn inter_store<D>(&self, destination: D, keys: Vec<String>) -> Result<usize>
    where
        D: AsRef<str>,
    {
        let payload = json!({
            "destination": destination.as_ref(),
            "keys": keys,
        });

        let response = self.client.send_command("set.interstore", payload).await?;
        Ok(response
            .get("cardinality")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize)
    }

    /// Store union result in destination
    pub async fn union_store<D>(&self, destination: D, keys: Vec<String>) -> Result<usize>
    where
        D: AsRef<str>,
    {
        let payload = json!({
            "destination": destination.as_ref(),
            "keys": keys,
        });

        let response = self.client.send_command("set.unionstore", payload).await?;
        Ok(response
            .get("cardinality")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize)
    }

    /// Store difference result in destination
    pub async fn diff_store<D>(&self, destination: D, keys: Vec<String>) -> Result<usize>
    where
        D: AsRef<str>,
    {
        let payload = json!({
            "destination": destination.as_ref(),
            "keys": keys,
        });

        let response = self.client.send_command("set.diffstore", payload).await?;
        Ok(response
            .get("cardinality")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize)
    }
}
