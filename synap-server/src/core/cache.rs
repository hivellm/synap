use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

/// L1/L2 Cache system for KV Store
/// L1: In-memory LRU cache (hot data)
/// L2: Optional disk-based cache (warm data) - future
#[derive(Clone)]
pub struct CacheLayer {
    /// L1 Cache - in-memory LRU
    l1: Arc<RwLock<LRUCache>>,

    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
}

/// LRU Cache implementation
struct LRUCache {
    /// Cache data
    data: HashMap<String, CacheEntry>,

    /// LRU ordering (most recent at back)
    lru_order: VecDeque<String>,

    /// Maximum capacity
    max_size: usize,
}

/// Cache entry
#[derive(Clone)]
struct CacheEntry {
    /// Cached value
    value: Vec<u8>,

    /// TTL (if any)
    ttl: Option<u64>,

    /// Last accessed timestamp
    last_accessed: u64,

    /// Size in bytes
    size: usize,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l1_evictions: u64,
    pub l1_size: usize,
    pub l1_entries: usize,
    pub total_bytes: usize,
}

impl CacheLayer {
    /// Create a new cache layer
    pub fn new(l1_max_size: usize) -> Self {
        Self {
            l1: Arc::new(RwLock::new(LRUCache {
                data: HashMap::new(),
                lru_order: VecDeque::new(),
                max_size: l1_max_size,
            })),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Get value from cache
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut l1 = self.l1.write();
        let mut stats = self.stats.write();

        // Check if entry exists and is valid
        let is_expired = if let Some(entry) = l1.data.get(key) {
            if let Some(ttl) = entry.ttl {
                Self::current_timestamp() > ttl
            } else {
                false
            }
        } else {
            stats.l1_misses += 1;
            debug!("L1 Cache MISS for key: {}", key);
            return None;
        };

        // If expired, remove and return None
        if is_expired {
            l1.data.remove(key);
            l1.lru_order.retain(|k| k != key);
            stats.l1_misses += 1;
            return None;
        }

        // Get the value and update access time
        if let Some(entry) = l1.data.get_mut(key) {
            entry.last_accessed = Self::current_timestamp();
            let value = entry.value.clone();

            // Move to back of LRU (most recent)
            let _ = entry; // Release the mutable borrow
            l1.lru_order.retain(|k| k != key);
            l1.lru_order.push_back(key.to_string());

            stats.l1_hits += 1;
            debug!("L1 Cache HIT for key: {}", key);

            Some(value)
        } else {
            None
        }
    }

    /// Put value into cache
    pub fn put(&self, key: String, value: Vec<u8>, ttl: Option<u64>) {
        let mut l1 = self.l1.write();
        let mut stats = self.stats.write();

        let entry_size = value.len();

        // If key exists, remove from LRU order
        if l1.data.contains_key(&key) {
            l1.lru_order.retain(|k| k != &key);
            if let Some(old_entry) = l1.data.get(&key) {
                stats.total_bytes = stats.total_bytes.saturating_sub(old_entry.size);
            }
        }

        // Evict if at capacity
        while l1.data.len() >= l1.max_size && !l1.lru_order.is_empty() {
            if let Some(evict_key) = l1.lru_order.pop_front() {
                if let Some(evicted) = l1.data.remove(&evict_key) {
                    stats.l1_evictions += 1;
                    stats.total_bytes = stats.total_bytes.saturating_sub(evicted.size);
                    debug!("L1 Cache EVICT: {}", evict_key);
                }
            }
        }

        // Insert new entry
        let entry = CacheEntry {
            value,
            ttl,
            last_accessed: Self::current_timestamp(),
            size: entry_size,
        };

        l1.data.insert(key.clone(), entry);
        l1.lru_order.push_back(key.clone());

        stats.l1_entries = l1.data.len();
        stats.l1_size = l1.max_size;
        stats.total_bytes += entry_size;

        debug!("L1 Cache PUT: {} ({} bytes)", key, entry_size);
    }

    /// Delete value from cache
    pub fn delete(&self, key: &str) {
        let mut l1 = self.l1.write();
        let mut stats = self.stats.write();

        if let Some(entry) = l1.data.remove(key) {
            l1.lru_order.retain(|k| k != key);
            stats.l1_entries = l1.data.len();
            stats.total_bytes = stats.total_bytes.saturating_sub(entry.size);
            debug!("L1 Cache DELETE: {}", key);
        }
    }

    /// Invalidate (clear) entire cache
    pub fn invalidate_all(&self) {
        let mut l1 = self.l1.write();
        let mut stats = self.stats.write();

        let count = l1.data.len();
        l1.data.clear();
        l1.lru_order.clear();

        stats.l1_entries = 0;
        stats.total_bytes = 0;

        debug!("L1 Cache INVALIDATE ALL ({} entries)", count);
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let stats = self.stats.read();
        let mut result = stats.clone();

        // Calculate hit rate
        let total = result.l1_hits + result.l1_misses;
        result
    }

    /// Get current Unix timestamp
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

impl Default for CacheLayer {
    fn default() -> Self {
        // Default L1 cache: 10,000 entries
        Self::new(10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_put_get() {
        let cache = CacheLayer::new(100);

        cache.put("key1".to_string(), vec![1, 2, 3], None);

        let value = cache.get("key1").unwrap();
        assert_eq!(value, vec![1, 2, 3]);

        let stats = cache.get_stats();
        assert_eq!(stats.l1_hits, 1);
        assert_eq!(stats.l1_misses, 0);
    }

    #[test]
    fn test_cache_miss() {
        let cache = CacheLayer::new(100);

        let value = cache.get("nonexistent");
        assert!(value.is_none());

        let stats = cache.get_stats();
        assert_eq!(stats.l1_hits, 0);
        assert_eq!(stats.l1_misses, 1);
    }

    #[test]
    fn test_cache_lru_eviction() {
        let cache = CacheLayer::new(3); // Small cache

        // Fill cache
        cache.put("key1".to_string(), vec![1], None);
        cache.put("key2".to_string(), vec![2], None);
        cache.put("key3".to_string(), vec![3], None);

        // Add one more - should evict key1 (oldest)
        cache.put("key4".to_string(), vec![4], None);

        assert!(cache.get("key1").is_none(), "key1 should be evicted");
        assert!(cache.get("key2").is_some(), "key2 should still exist");
        assert!(cache.get("key3").is_some(), "key3 should still exist");
        assert!(cache.get("key4").is_some(), "key4 should exist");

        let stats = cache.get_stats();
        assert_eq!(stats.l1_evictions, 1);
    }

    #[test]
    fn test_cache_ttl_expiration() {
        let cache = CacheLayer::new(100);

        let past_ttl = CacheLayer::current_timestamp() - 10; // 10 seconds ago
        cache.put("expired".to_string(), vec![1, 2, 3], Some(past_ttl));

        // Should return None (expired)
        let value = cache.get("expired");
        assert!(value.is_none());
    }

    #[test]
    fn test_cache_delete() {
        let cache = CacheLayer::new(100);

        cache.put("key1".to_string(), vec![1, 2, 3], None);
        assert!(cache.get("key1").is_some());

        cache.delete("key1");
        assert!(cache.get("key1").is_none());
    }

    #[test]
    fn test_cache_invalidate_all() {
        let cache = CacheLayer::new(100);

        cache.put("key1".to_string(), vec![1], None);
        cache.put("key2".to_string(), vec![2], None);
        cache.put("key3".to_string(), vec![3], None);

        cache.invalidate_all();

        assert!(cache.get("key1").is_none());
        assert!(cache.get("key2").is_none());
        assert!(cache.get("key3").is_none());

        let stats = cache.get_stats();
        assert_eq!(stats.l1_entries, 0);
    }

    #[test]
    fn test_cache_lru_order() {
        let cache = CacheLayer::new(3);

        cache.put("key1".to_string(), vec![1], None);
        cache.put("key2".to_string(), vec![2], None);
        cache.put("key3".to_string(), vec![3], None);

        // Access key1 (moves to back)
        cache.get("key1");

        // Add key4 - should evict key2 (now oldest)
        cache.put("key4".to_string(), vec![4], None);

        assert!(
            cache.get("key1").is_some(),
            "key1 was accessed, should not be evicted"
        );
        assert!(
            cache.get("key2").is_none(),
            "key2 should be evicted (oldest)"
        );
    }
}
