use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Stored value in the KV store with compact metadata
/// Memory-optimized: eliminates 48 bytes overhead vs old struct
#[derive(Debug, Clone)]
pub enum StoredValue {
    /// Persistent value without TTL (24 bytes overhead only)
    Persistent(Vec<u8>),
    
    /// Expiring value with TTL and LRU tracking (32 bytes overhead)
    Expiring {
        data: Vec<u8>,
        expires_at: u32,   // Unix timestamp (valid until year 2106)
        last_access: u32,  // Unix timestamp for LRU
    },
}

impl StoredValue {
    /// Create a new stored value
    pub fn new(data: Vec<u8>, ttl_secs: Option<u64>) -> Self {
        match ttl_secs {
            None => Self::Persistent(data),
            Some(secs) => {
                let now = Self::current_timestamp();
                Self::Expiring {
                    data,
                    expires_at: now.saturating_add(secs as u32),
                    last_access: now,
                }
            }
        }
    }

    /// Get current Unix timestamp as u32
    #[inline]
    fn current_timestamp() -> u32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0)
    }

    /// Check if the value has expired
    #[inline]
    pub fn is_expired(&self) -> bool {
        match self {
            Self::Persistent(_) => false,
            Self::Expiring { expires_at, .. } => {
                Self::current_timestamp() >= *expires_at
            }
        }
    }

    /// Update access time for LRU
    pub fn update_access(&mut self) {
        if let Self::Expiring { last_access, .. } = self {
            *last_access = Self::current_timestamp();
        }
    }

    /// Get remaining TTL in seconds
    pub fn remaining_ttl_secs(&self) -> Option<u64> {
        match self {
            Self::Persistent(_) => None,
            Self::Expiring { expires_at, .. } => {
                let now = Self::current_timestamp();
                if now >= *expires_at {
                    Some(0)
                } else {
                    Some((*expires_at - now) as u64)
                }
            }
        }
    }

    /// Get reference to data regardless of variant
    #[inline]
    pub fn data(&self) -> &[u8] {
        match self {
            Self::Persistent(data) => data,
            Self::Expiring { data, .. } => data,
        }
    }

    /// Get mutable reference to data regardless of variant
    #[inline]
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        match self {
            Self::Persistent(data) => data,
            Self::Expiring { data, .. } => data,
        }
    }

    /// Get last access timestamp (for LRU eviction)
    pub fn last_access(&self) -> u32 {
        match self {
            Self::Persistent(_) => 0,
            Self::Expiring { last_access, .. } => *last_access,
        }
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
    /// Allow dangerous FLUSH commands (disabled by default like Redis)
    pub allow_flush_commands: bool,
}

impl Default for KVConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 4096,
            eviction_policy: EvictionPolicy::Lru,
            ttl_cleanup_interval_ms: 100,
            allow_flush_commands: false,  // Disabled by default for safety
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
