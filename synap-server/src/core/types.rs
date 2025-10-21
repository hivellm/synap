use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Stored value in the KV store with metadata
#[derive(Debug, Clone)]
pub struct StoredValue {
    /// Raw byte data
    pub data: Vec<u8>,
    /// Optional expiration time
    pub ttl: Option<Instant>,
    /// When the value was created
    pub created_at: Instant,
    /// Last access time (for LRU)
    pub accessed_at: Instant,
}

impl StoredValue {
    /// Create a new stored value
    pub fn new(data: Vec<u8>, ttl_secs: Option<u64>) -> Self {
        let now = Instant::now();
        Self {
            data,
            ttl: ttl_secs.map(|secs| now + std::time::Duration::from_secs(secs)),
            created_at: now,
            accessed_at: now,
        }
    }

    /// Check if the value has expired
    pub fn is_expired(&self) -> bool {
        self.ttl.is_some_and(|expires| Instant::now() >= expires)
    }

    /// Update access time
    pub fn update_access(&mut self) {
        self.accessed_at = Instant::now();
    }

    /// Get remaining TTL in seconds
    pub fn remaining_ttl_secs(&self) -> Option<u64> {
        self.ttl.map(|expires| {
            let now = Instant::now();
            if now >= expires {
                0
            } else {
                (expires - now).as_secs()
            }
        })
    }
}

/// Eviction policy for memory management
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EvictionPolicy {
    /// No eviction, return error when full
    None,
    /// Least Recently Used
    #[default]
    Lru,
    /// Least Frequently Used
    Lfu,
    /// Evict keys with shortest TTL first
    Ttl,
}

/// Configuration for KV store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVConfig {
    /// Maximum memory in MB
    pub max_memory_mb: usize,
    /// Eviction policy when memory limit reached
    pub eviction_policy: EvictionPolicy,
    /// TTL cleanup interval in milliseconds
    pub ttl_cleanup_interval_ms: u64,
}

impl Default for KVConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 4096,
            eviction_policy: EvictionPolicy::Lru,
            ttl_cleanup_interval_ms: 100,
        }
    }
}

/// Statistics for KV store
#[derive(Debug, Default, Clone, Serialize)]
pub struct KVStats {
    /// Total number of keys
    pub total_keys: usize,
    /// Estimated memory usage in bytes
    pub total_memory_bytes: usize,
    /// Number of GET operations
    pub gets: u64,
    /// Number of SET operations
    pub sets: u64,
    /// Number of DELETE operations
    pub dels: u64,
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
}

impl KVStats {
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}
