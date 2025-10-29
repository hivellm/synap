use super::error::{Result, SynapError};
use super::{HashStore, KVStore, ListStore, SetStore, SortedSetStore};
use rand::Rng;
use std::sync::Arc;
use tracing::{debug, warn};

/// Key type enumeration (Redis-compatible)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum KeyType {
    #[serde(rename = "string")]
    String,
    #[serde(rename = "hash")]
    Hash,
    #[serde(rename = "list")]
    List,
    #[serde(rename = "set")]
    Set,
    #[serde(rename = "zset")]
    SortedSet,
    #[serde(rename = "none")]
    None,
}

impl KeyType {
    /// Get Redis-style type string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Hash => "hash",
            Self::List => "list",
            Self::Set => "set",
            Self::SortedSet => "zset",
            Self::None => "none",
        }
    }
}

/// Key Manager for cross-store operations
#[derive(Clone)]
pub struct KeyManager {
    kv_store: Arc<KVStore>,
    hash_store: Arc<HashStore>,
    list_store: Arc<ListStore>,
    set_store: Arc<SetStore>,
    sorted_set_store: Arc<SortedSetStore>,
}

impl KeyManager {
    /// Create a new KeyManager with references to all stores
    pub fn new(
        kv_store: Arc<KVStore>,
        hash_store: Arc<HashStore>,
        list_store: Arc<ListStore>,
        set_store: Arc<SetStore>,
        sorted_set_store: Arc<SortedSetStore>,
    ) -> Self {
        Self {
            kv_store,
            hash_store,
            list_store,
            set_store,
            sorted_set_store,
        }
    }

    /// TYPE: Get the type of a key
    /// Returns: "string", "hash", "list", "set", "zset", or "none"
    pub async fn key_type(&self, key: &str) -> Result<KeyType> {
        debug!("TYPE key={}", key);

        // Check in order: SortedSet, Set, List, Hash, KV (order matters for performance)
        // Check most specific types first (zset, set, list, hash) then fallback to string

        // For sorted set, check if key has any members by checking zcard
        if self.sorted_set_store.zcard(key) > 0 {
            return Ok(KeyType::SortedSet);
        }

        // For set, check if key exists by checking scard
        if self.set_store.scard(key).unwrap_or(0) > 0 {
            return Ok(KeyType::Set);
        }

        // For list, check if key exists by checking llen
        if self
            .list_store
            .llen(key)
            .map(|len| len > 0)
            .unwrap_or(false)
        {
            return Ok(KeyType::List);
        }

        // For hash, check if key exists by checking hlen
        if self
            .hash_store
            .hlen(key)
            .map(|len| len > 0)
            .unwrap_or(false)
        {
            return Ok(KeyType::Hash);
        }

        // Check KV store last (most generic)
        if self.kv_store.exists(key).await? {
            return Ok(KeyType::String);
        }

        Ok(KeyType::None)
    }

    /// EXISTS: Check if a key exists in any store
    pub async fn exists(&self, key: &str) -> Result<bool> {
        debug!("EXISTS key={}", key);

        // Check all stores
        if self.kv_store.exists(key).await? {
            return Ok(true);
        }
        if self
            .hash_store
            .hlen(key)
            .map(|len| len > 0)
            .unwrap_or(false)
        {
            return Ok(true);
        }
        if self
            .list_store
            .llen(key)
            .map(|len| len > 0)
            .unwrap_or(false)
        {
            return Ok(true);
        }
        if self.set_store.scard(key).unwrap_or(0) > 0 {
            return Ok(true);
        }
        if self.sorted_set_store.zcard(key) > 0 {
            return Ok(true);
        }

        Ok(false)
    }

    /// RENAME: Rename a key atomically, overwriting destination if it exists
    pub async fn rename(&self, source: &str, destination: &str) -> Result<()> {
        debug!("RENAME source={}, destination={}", source, destination);

        if source == destination {
            return Ok(()); // No-op if same key
        }

        let key_type = self.key_type(source).await?;

        if key_type == KeyType::None {
            return Err(SynapError::KeyNotFound(source.to_string()));
        }

        // Delete destination if it exists (RENAME overwrites)
        if self.exists(destination).await? {
            self.delete(destination).await?;
        }

        // Perform rename based on type
        match key_type {
            KeyType::String => {
                // Get value from KV store
                let value = self
                    .kv_store
                    .get(source)
                    .await?
                    .ok_or_else(|| SynapError::KeyNotFound(source.to_string()))?;

                // Get TTL
                let ttl = self.kv_store.ttl(source).await?;

                // Set in destination
                self.kv_store.set(destination, value, ttl).await?;

                // Delete source
                self.kv_store.delete(source).await?;
            }
            KeyType::Hash => {
                // Get all fields
                let all_fields = self.hash_store.hgetall(source)?;

                // Set in destination
                for (field, value) in all_fields {
                    self.hash_store.hset(destination, &field, value)?;
                }

                // Delete source - get keys first
                let keys = self.hash_store.hkeys(source)?;
                if !keys.is_empty() {
                    self.hash_store.hdel(source, &keys)?;
                }
            }
            KeyType::List => {
                // Get all values (via LRANGE)
                let values = self.list_store.lrange(source, 0, -1)?;

                // Create new list in destination
                self.list_store.rpush(destination, values, false)?;

                // Delete source by clearing list
                self.list_store.ltrim(source, 1, 0)?;
            }
            KeyType::Set => {
                // Get all members
                let members = self.set_store.smembers(source)?;

                // Add to destination
                if !members.is_empty() {
                    self.set_store.sadd(destination, members)?;
                }

                // Delete source - get members again
                let all_members = self.set_store.smembers(source)?;
                if !all_members.is_empty() {
                    self.set_store.srem(source, all_members)?;
                }
            }
            KeyType::SortedSet => {
                // Get all members with scores (via ZRANGE with scores)
                let members = self.sorted_set_store.zrange(source, 0, -1, true);

                // Add to destination
                for member in members {
                    self.sorted_set_store.zadd(
                        destination,
                        member.member,
                        member.score,
                        &Default::default(),
                    );
                }

                // Delete source - get all members first
                let all_members = self.sorted_set_store.zrange(source, 0, -1, false);
                let member_bytes: Vec<Vec<u8>> =
                    all_members.into_iter().map(|m| m.member).collect();
                if !member_bytes.is_empty() {
                    let _ = self.sorted_set_store.zrem(source, &member_bytes);
                }
            }
            KeyType::None => {
                return Err(SynapError::KeyNotFound(source.to_string()));
            }
        }

        Ok(())
    }

    /// RENAMENX: Rename key only if destination doesn't exist
    /// Returns: true if renamed, false if destination exists
    pub async fn renamenx(&self, source: &str, destination: &str) -> Result<bool> {
        debug!("RENAMENX source={}, destination={}", source, destination);

        if source == destination {
            return Ok(true); // Same key, consider it success
        }

        // Check if destination exists
        if self.exists(destination).await? {
            return Ok(false);
        }

        // Perform rename (destination guaranteed not to exist)
        self.rename(source, destination).await?;
        Ok(true)
    }

    /// COPY: Copy key to destination, preserving TTL if requested
    pub async fn copy(&self, source: &str, destination: &str, replace: bool) -> Result<bool> {
        debug!(
            "COPY source={}, destination={}, replace={}",
            source, destination, replace
        );

        if source == destination {
            return Ok(true); // Copying to self is no-op
        }

        let key_type = self.key_type(source).await?;

        if key_type == KeyType::None {
            return Err(SynapError::KeyNotFound(source.to_string()));
        }

        // Check if destination exists
        if self.exists(destination).await? {
            if !replace {
                return Ok(false); // Destination exists and replace=false
            }
            // Delete destination if replace=true
            self.delete(destination).await?;
        }

        // Perform copy based on type
        match key_type {
            KeyType::String => {
                let value = self
                    .kv_store
                    .get(source)
                    .await?
                    .ok_or_else(|| SynapError::KeyNotFound(source.to_string()))?;

                let ttl = self.kv_store.ttl(source).await?;
                self.kv_store.set(destination, value, ttl).await?;
            }
            KeyType::Hash => {
                let all_fields = self.hash_store.hgetall(source)?;
                for (field, value) in all_fields {
                    self.hash_store.hset(destination, &field, value)?;
                }
            }
            KeyType::List => {
                let values = self.list_store.lrange(source, 0, -1)?;
                self.list_store.rpush(destination, values, false)?;
            }
            KeyType::Set => {
                let members = self.set_store.smembers(source)?;
                if !members.is_empty() {
                    self.set_store.sadd(destination, members)?;
                }
            }
            KeyType::SortedSet => {
                let members = self.sorted_set_store.zrange(source, 0, -1, true);
                for member in members {
                    self.sorted_set_store.zadd(
                        destination,
                        member.member,
                        member.score,
                        &Default::default(),
                    );
                }
            }
            KeyType::None => {
                return Err(SynapError::KeyNotFound(source.to_string()));
            }
        }

        Ok(true)
    }

    /// RANDOMKEY: Get a random key from any store
    pub async fn randomkey(&self) -> Result<Option<String>> {
        debug!("RANDOMKEY");

        // Try to get a random key from each store in order
        // Start with most common types

        // Try KV store first (most common)
        let kv_keys = self.kv_store.keys().await?;
        if !kv_keys.is_empty() {
            let mut rng = rand::rngs::ThreadRng::default();
            let idx = rng.random_range(0..kv_keys.len());
            return Ok(Some(kv_keys[idx].clone()));
        }

        // Try Hash store
        // Note: Hash stores don't have a direct keys() method, would need to track keys
        // For now, skip hash/list/set/sortedset as they don't expose key enumeration

        // For this implementation, we'll focus on KV store
        // Other stores would need key tracking to be added
        warn!("RANDOMKEY: Only KV store keys supported, other stores not yet tracked");

        Ok(None)
    }

    /// Delete a key from any store
    async fn delete(&self, key: &str) -> Result<()> {
        let key_type = self.key_type(key).await?;

        match key_type {
            KeyType::String => {
                self.kv_store.delete(key).await?;
            }
            KeyType::Hash => {
                let all_fields = self.hash_store.hkeys(key)?;
                if !all_fields.is_empty() {
                    self.hash_store.hdel(key, &all_fields)?;
                }
            }
            KeyType::List => {
                self.list_store.ltrim(key, 1, 0)?; // Clear list
            }
            KeyType::Set => {
                let all_members = self.set_store.smembers(key)?;
                if !all_members.is_empty() {
                    self.set_store.srem(key, all_members)?;
                }
            }
            KeyType::SortedSet => {
                // Get all members and remove them
                let members = self.sorted_set_store.zrange(key, 0, -1, false);
                let member_bytes: Vec<Vec<u8>> = members.into_iter().map(|m| m.member).collect();
                if !member_bytes.is_empty() {
                    let _ = self.sorted_set_store.zrem(key, &member_bytes);
                }
            }
            KeyType::None => {
                // Key doesn't exist, that's fine
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{HashStore, KVConfig, ListStore, SetStore, SortedSetStore};

    #[tokio::test]
    async fn test_key_type() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        // Test string type
        kv_store
            .set("key1", b"value1".to_vec(), None)
            .await
            .unwrap();
        assert_eq!(manager.key_type("key1").await.unwrap(), KeyType::String);

        // Test hash type
        hash_store.hset("key2", "field", b"value".to_vec()).unwrap();
        assert_eq!(manager.key_type("key2").await.unwrap(), KeyType::Hash);

        // Test none type
        assert_eq!(
            manager.key_type("nonexistent").await.unwrap(),
            KeyType::None
        );
    }

    #[tokio::test]
    async fn test_exists() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        kv_store
            .set("key1", b"value1".to_vec(), None)
            .await
            .unwrap();
        assert!(manager.exists("key1").await.unwrap());

        assert!(!manager.exists("nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_rename_kv() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        kv_store.set("old", b"value".to_vec(), None).await.unwrap();
        manager.rename("old", "new").await.unwrap();

        assert!(kv_store.exists("new").await.unwrap());
        assert!(!kv_store.exists("old").await.unwrap());

        let value = kv_store.get("new").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_renamenx() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        kv_store
            .set("source", b"value".to_vec(), None)
            .await
            .unwrap();
        kv_store
            .set("dest", b"existing".to_vec(), None)
            .await
            .unwrap();

        // Should fail because dest exists
        let result = manager.renamenx("source", "dest").await.unwrap();
        assert!(!result);

        // Source should still exist
        assert!(kv_store.exists("source").await.unwrap());
        assert_eq!(
            kv_store.get("dest").await.unwrap(),
            Some(b"existing".to_vec())
        );

        // Delete dest and try again
        kv_store.delete("dest").await.unwrap();
        let result = manager.renamenx("source", "dest").await.unwrap();
        assert!(result);

        assert!(!kv_store.exists("source").await.unwrap());
        assert!(kv_store.exists("dest").await.unwrap());
    }

    #[tokio::test]
    async fn test_copy_kv() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        kv_store
            .set("source", b"value".to_vec(), None)
            .await
            .unwrap();

        // Copy without replace when dest doesn't exist
        let result = manager.copy("source", "dest", false).await.unwrap();
        assert!(result);

        assert!(kv_store.exists("source").await.unwrap());
        assert!(kv_store.exists("dest").await.unwrap());
        assert_eq!(
            kv_store.get("source").await.unwrap(),
            kv_store.get("dest").await.unwrap()
        );

        // Copy with replace when dest exists
        kv_store.set("dest", b"old".to_vec(), None).await.unwrap();
        let result = manager.copy("source", "dest", true).await.unwrap();
        assert!(result);
        assert_eq!(kv_store.get("dest").await.unwrap(), Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_key_type_hash() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        hash_store
            .hset("key1", "field1", b"value1".to_vec())
            .unwrap();
        assert_eq!(manager.key_type("key1").await.unwrap(), KeyType::Hash);
    }

    #[tokio::test]
    async fn test_key_type_list() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        list_store
            .rpush("key1", vec![b"value1".to_vec()], false)
            .unwrap();
        assert_eq!(manager.key_type("key1").await.unwrap(), KeyType::List);
    }

    #[tokio::test]
    async fn test_key_type_set() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        set_store.sadd("key1", vec![b"member1".to_vec()]).unwrap();
        assert_eq!(manager.key_type("key1").await.unwrap(), KeyType::Set);
    }

    #[tokio::test]
    async fn test_key_type_sortedset() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        sorted_set_store.zadd("key1", b"member1".to_vec(), 1.0, &Default::default());
        assert_eq!(manager.key_type("key1").await.unwrap(), KeyType::SortedSet);
    }

    #[tokio::test]
    async fn test_rename_hash() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        hash_store
            .hset("old", "field1", b"value1".to_vec())
            .unwrap();
        manager.rename("old", "new").await.unwrap();

        assert!(hash_store.hlen("new").unwrap() > 0);
        assert_eq!(hash_store.hlen("old").unwrap(), 0);
    }

    #[tokio::test]
    async fn test_copy_hash() {
        let kv_store = Arc::new(KVStore::new(KVConfig::default()));
        let hash_store = Arc::new(HashStore::new());
        let list_store = Arc::new(ListStore::new());
        let set_store = Arc::new(SetStore::new());
        let sorted_set_store = Arc::new(SortedSetStore::new());

        let manager = KeyManager::new(
            kv_store.clone(),
            hash_store.clone(),
            list_store.clone(),
            set_store.clone(),
            sorted_set_store.clone(),
        );

        hash_store
            .hset("source", "field1", b"value1".to_vec())
            .unwrap();
        manager.copy("source", "dest", false).await.unwrap();

        assert!(hash_store.hlen("source").unwrap() > 0);
        assert!(hash_store.hlen("dest").unwrap() > 0);
        assert_eq!(
            hash_store.hget("source", "field1").unwrap(),
            hash_store.hget("dest", "field1").unwrap()
        );
    }
}
