use super::error::{Result, SynapError};
use super::types::{KVConfig, KVStats, StoredValue};
use parking_lot::RwLock;
use radix_trie::{Trie, TrieCommon};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Key-Value store using radix trie for memory-efficient storage
#[derive(Clone)]
pub struct KVStore {
    data: Arc<RwLock<Trie<String, StoredValue>>>,
    stats: Arc<RwLock<KVStats>>,
    config: KVConfig,
}

impl KVStore {
    /// Create a new KV store with the given configuration
    pub fn new(config: KVConfig) -> Self {
        info!(
            "Initializing KV store with max_memory={}MB, eviction={:?}",
            config.max_memory_mb, config.eviction_policy
        );

        Self {
            data: Arc::new(RwLock::new(Trie::new())),
            stats: Arc::new(RwLock::new(KVStats::default())),
            config,
        }
    }

    /// Start background TTL cleanup task
    pub fn start_ttl_cleanup(&self) -> tokio::task::JoinHandle<()> {
        let interval_ms = self.config.ttl_cleanup_interval_ms;
        info!("Starting TTL cleanup task (interval={}ms)", interval_ms);

        let store = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));

            loop {
                interval.tick().await;
                store.cleanup_expired().await;
            }
        })
    }

    /// Set a key-value pair
    pub async fn set(&self, key: &str, value: Vec<u8>, ttl_secs: Option<u64>) -> Result<()> {
        debug!("SET key={}, size={}, ttl={:?}", key, value.len(), ttl_secs);

        let stored = StoredValue::new(value, ttl_secs);
        let entry_size = self.estimate_entry_size(key, &stored);

        // Check memory limits
        {
            let stats = self.stats.read();
            let max_bytes = self.config.max_memory_mb * 1024 * 1024;
            if stats.total_memory_bytes + entry_size > max_bytes {
                warn!(
                    "Memory limit exceeded: {}/{}",
                    stats.total_memory_bytes, max_bytes
                );
                return Err(SynapError::MemoryLimitExceeded);
            }
        }

        // Insert value
        let mut data = self.data.write();
        let is_new = data.insert(key.to_string(), stored).is_none();

        // Update stats
        let mut stats = self.stats.write();
        stats.sets += 1;
        if is_new {
            stats.total_keys += 1;
            stats.total_memory_bytes += entry_size;
        }

        Ok(())
    }

    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        debug!("GET key={}", key);

        let mut data = self.data.write();
        let mut stats = self.stats.write();
        stats.gets += 1;

        if let Some(value) = data.get_mut(key) {
            // Check if expired
            if value.is_expired() {
                debug!("Key expired: {}", key);
                data.remove(key);
                stats.misses += 1;
                stats.total_keys = stats.total_keys.saturating_sub(1);
                return Ok(None);
            }

            // Update access time for LRU
            value.update_access();
            stats.hits += 1;
            Ok(Some(value.data.clone()))
        } else {
            stats.misses += 1;
            Ok(None)
        }
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> Result<bool> {
        debug!("DELETE key={}", key);

        let mut data = self.data.write();
        let removed = data.remove(key);

        if removed.is_some() {
            let mut stats = self.stats.write();
            stats.dels += 1;
            stats.total_keys = stats.total_keys.saturating_sub(1);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if a key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let data = self.data.read();
        if let Some(value) = data.get(key) {
            Ok(!value.is_expired())
        } else {
            Ok(false)
        }
    }

    /// Get statistics
    pub async fn stats(&self) -> KVStats {
        self.stats.read().clone()
    }

    /// Get remaining TTL for a key
    pub async fn ttl(&self, key: &str) -> Result<Option<u64>> {
        let data = self.data.read();
        if let Some(value) = data.get(key) {
            if value.is_expired() {
                Ok(Some(0))
            } else {
                Ok(value.remaining_ttl_secs())
            }
        } else {
            Err(SynapError::KeyNotFound(key.to_string()))
        }
    }

    /// Atomic increment
    pub async fn incr(&self, key: &str, amount: i64) -> Result<i64> {
        debug!("INCR key={}, amount={}", key, amount);

        let mut data = self.data.write();

        let current_value = if let Some(value) = data.get(key) {
            if value.is_expired() {
                0
            } else {
                String::from_utf8(value.data.clone())
                    .ok()
                    .and_then(|s| s.parse::<i64>().ok())
                    .ok_or_else(|| {
                        SynapError::InvalidValue("Value is not a valid integer".to_string())
                    })?
            }
        } else {
            0
        };

        let new_value = current_value + amount;
        let new_data = new_value.to_string().into_bytes();

        data.insert(key.to_string(), StoredValue::new(new_data, None));

        let mut stats = self.stats.write();
        stats.sets += 1;

        Ok(new_value)
    }

    /// Atomic decrement
    pub async fn decr(&self, key: &str, amount: i64) -> Result<i64> {
        self.incr(key, -amount).await
    }

    /// Set multiple key-value pairs
    pub async fn mset(&self, pairs: Vec<(String, Vec<u8>)>) -> Result<()> {
        debug!("MSET count={}", pairs.len());

        for (key, value) in pairs {
            self.set(&key, value, None).await?;
        }

        Ok(())
    }

    /// Get multiple values
    pub async fn mget(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        debug!("MGET count={}", keys.len());

        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(self.get(key).await?);
        }

        Ok(results)
    }

    /// Delete multiple keys
    pub async fn mdel(&self, keys: &[String]) -> Result<usize> {
        debug!("MDEL count={}", keys.len());

        let mut count = 0;
        for key in keys {
            if self.delete(key).await? {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Scan keys with optional prefix
    pub async fn scan(&self, prefix: Option<&str>, limit: usize) -> Result<Vec<String>> {
        debug!("SCAN prefix={:?}, limit={}", prefix, limit);

        let data = self.data.read();
        let keys: Vec<String> = if let Some(prefix) = prefix {
            data.get_raw_descendant(prefix)
                .map(|subtrie| subtrie.keys().map(|k| k.to_string()).take(limit).collect())
                .unwrap_or_default()
        } else {
            data.keys().map(|k| k.to_string()).take(limit).collect()
        };

        Ok(keys)
    }

    /// Clean up expired keys
    async fn cleanup_expired(&self) {
        let mut data = self.data.write();
        let mut stats = self.stats.write();

        let expired_keys: Vec<String> = data
            .iter()
            .filter(|(_, v)| v.is_expired())
            .map(|(k, _)| k.clone())
            .collect();

        let count = expired_keys.len();
        if count > 0 {
            debug!("Cleaning up {} expired keys", count);
            for key in expired_keys {
                data.remove(&key);
            }
            stats.total_keys = stats.total_keys.saturating_sub(count);
        }
    }

    /// Estimate memory size of an entry
    fn estimate_entry_size(&self, key: &str, value: &StoredValue) -> usize {
        key.len() + value.data.len() + std::mem::size_of::<StoredValue>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_get() {
        let store = KVStore::new(KVConfig::default());

        // Set a value
        store.set("key1", b"value1".to_vec(), None).await.unwrap();

        // Get the value
        let result = store.get("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let store = KVStore::new(KVConfig::default());

        let result = store.get("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = KVStore::new(KVConfig::default());

        store.set("key1", b"value1".to_vec(), None).await.unwrap();

        let deleted = store.delete("key1").await.unwrap();
        assert!(deleted);

        let result = store.get("key1").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let store = KVStore::new(KVConfig::default());

        // Set with 1 second TTL
        store
            .set("key1", b"value1".to_vec(), Some(1))
            .await
            .unwrap();

        // Should exist initially
        let result = store.get("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));

        // Wait for expiration
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should be expired
        let result = store.get("key1").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_exists() {
        let store = KVStore::new(KVConfig::default());

        store.set("key1", b"value1".to_vec(), None).await.unwrap();

        assert!(store.exists("key1").await.unwrap());
        assert!(!store.exists("key2").await.unwrap());
    }

    #[tokio::test]
    async fn test_incr() {
        let store = KVStore::new(KVConfig::default());

        let val = store.incr("counter", 1).await.unwrap();
        assert_eq!(val, 1);

        let val = store.incr("counter", 5).await.unwrap();
        assert_eq!(val, 6);
    }

    #[tokio::test]
    async fn test_decr() {
        let store = KVStore::new(KVConfig::default());

        let val = store.incr("counter", 10).await.unwrap();
        assert_eq!(val, 10);

        let val = store.decr("counter", 3).await.unwrap();
        assert_eq!(val, 7);
    }

    #[tokio::test]
    async fn test_mset_mget() {
        let store = KVStore::new(KVConfig::default());

        let pairs = vec![
            ("key1".to_string(), b"value1".to_vec()),
            ("key2".to_string(), b"value2".to_vec()),
            ("key3".to_string(), b"value3".to_vec()),
        ];

        store.mset(pairs).await.unwrap();

        let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
        let results = store.mget(&keys).await.unwrap();

        assert_eq!(results[0], Some(b"value1".to_vec()));
        assert_eq!(results[1], Some(b"value2".to_vec()));
        assert_eq!(results[2], Some(b"value3".to_vec()));
    }

    #[tokio::test]
    async fn test_mdel() {
        let store = KVStore::new(KVConfig::default());

        store.set("key1", b"value1".to_vec(), None).await.unwrap();
        store.set("key2", b"value2".to_vec(), None).await.unwrap();
        store.set("key3", b"value3".to_vec(), None).await.unwrap();

        let keys = vec!["key1".to_string(), "key2".to_string(), "key4".to_string()];
        let count = store.mdel(&keys).await.unwrap();

        assert_eq!(count, 2);
        assert!(!store.exists("key1").await.unwrap());
        assert!(!store.exists("key2").await.unwrap());
        assert!(store.exists("key3").await.unwrap());
    }

    #[tokio::test]
    async fn test_scan() {
        let store = KVStore::new(KVConfig::default());

        store.set("user:1", b"alice".to_vec(), None).await.unwrap();
        store.set("user:2", b"bob".to_vec(), None).await.unwrap();
        store
            .set("product:1", b"laptop".to_vec(), None)
            .await
            .unwrap();

        let keys = store.scan(Some("user:"), 10).await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"user:1".to_string()));
        assert!(keys.contains(&"user:2".to_string()));
    }

    #[tokio::test]
    async fn test_stats() {
        let store = KVStore::new(KVConfig::default());

        store.set("key1", b"value1".to_vec(), None).await.unwrap();
        store.get("key1").await.unwrap();
        store.get("key2").await.unwrap();

        let stats = store.stats().await;
        assert_eq!(stats.sets, 1);
        assert_eq!(stats.gets, 2);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.total_keys, 1);
    }
}
