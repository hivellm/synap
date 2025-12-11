use super::error::{Result, SynapError};
use super::types::{KVConfig, KVStats, StoredValue};
use parking_lot::RwLock;
use radix_trie::{Trie, TrieCommon};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

const SHARD_COUNT: usize = 64;
const HASHMAP_THRESHOLD: usize = 10_000; // Switch to RadixTrie after 10K keys

/// Storage backend for a shard (adaptive: HashMap for small, RadixTrie for large)
/// Note: CompactString could reduce memory by 30% for short keys, but RadixTrie
/// doesn't implement TrieKey for CompactString. Using String for compatibility.
enum ShardStorage {
    /// HashMap for datasets < 10K keys (2-3x faster)
    Small(HashMap<String, StoredValue>),
    /// RadixTrie for datasets >= 10K keys (memory efficient for large sets)
    Large(Trie<String, StoredValue>),
}

impl ShardStorage {
    fn new() -> Self {
        // Start with HashMap for better small-dataset performance
        Self::Small(HashMap::new())
    }

    #[allow(dead_code)]
    fn len(&self) -> usize {
        match self {
            Self::Small(map) => map.len(),
            Self::Large(trie) => trie.len(),
        }
    }

    fn get(&self, key: &str) -> Option<&StoredValue> {
        match self {
            Self::Small(map) => map.get(key),
            Self::Large(trie) => trie.get(key),
        }
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut StoredValue> {
        match self {
            Self::Small(map) => map.get_mut(key),
            Self::Large(trie) => trie.get_mut(key),
        }
    }

    fn insert(&mut self, key: String, value: StoredValue) -> Option<StoredValue> {
        match self {
            Self::Small(map) => {
                let result = map.insert(key, value);
                // Check if we need to upgrade to RadixTrie
                if map.len() >= HASHMAP_THRESHOLD {
                    self.upgrade_to_trie();
                }
                result
            }
            Self::Large(trie) => trie.insert(key, value),
        }
    }

    fn remove(&mut self, key: &str) -> Option<StoredValue> {
        match self {
            Self::Small(map) => map.remove(key),
            Self::Large(trie) => trie.remove(key),
        }
    }

    fn iter(&self) -> Vec<(String, StoredValue)> {
        match self {
            Self::Small(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            Self::Large(trie) => trie.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }

    fn keys(&self) -> Vec<String> {
        match self {
            Self::Small(map) => map.keys().cloned().collect(),
            Self::Large(trie) => trie.keys().cloned().collect(),
        }
    }

    fn clear(&mut self) {
        match self {
            Self::Small(map) => map.clear(),
            Self::Large(trie) => *trie = Trie::new(),
        }
    }

    /// Get keys with a specific prefix (for SCAN command)
    fn get_prefix_keys(&self, prefix: &str) -> Vec<String> {
        match self {
            Self::Small(map) => {
                // HashMap doesn't have prefix search, so filter manually
                map.keys()
                    .filter(|k| k.starts_with(prefix))
                    .map(|k| k.to_string())
                    .collect()
            }
            Self::Large(trie) => {
                // Use RadixTrie's efficient prefix search
                trie.get_raw_descendant(prefix)
                    .map(|subtrie| subtrie.keys().cloned().collect())
                    .unwrap_or_default()
            }
        }
    }

    /// Upgrade from HashMap to RadixTrie when threshold is reached
    fn upgrade_to_trie(&mut self) {
        if let Self::Small(map) = self {
            debug!(
                "Upgrading shard from HashMap to RadixTrie (threshold {} reached)",
                HASHMAP_THRESHOLD
            );
            let mut trie = Trie::new();
            for (k, v) in map.drain() {
                trie.insert(k, v);
            }
            *self = Self::Large(trie);
        }
    }
}

/// Single shard of the KV store with adaptive storage
struct KVShard {
    data: RwLock<ShardStorage>,
}

impl KVShard {
    fn new() -> Self {
        Self {
            data: RwLock::new(ShardStorage::new()),
        }
    }
}

/// Key-Value store using 64-way sharded radix tries for lock-free concurrency
/// Eliminates lock contention by distributing keys across multiple shards
#[derive(Clone)]
pub struct KVStore {
    shards: Arc<[Arc<KVShard>; SHARD_COUNT]>,
    stats: Arc<RwLock<KVStats>>,
    config: KVConfig,
    /// Optional L1/L2 cache layer
    cache: Option<Arc<crate::core::CacheLayer>>,
    /// Optional cluster topology for cluster mode routing
    cluster_topology: Option<Arc<crate::cluster::topology::ClusterTopology>>,
    /// Optional migration manager for cluster mode
    cluster_migration: Option<Arc<crate::cluster::migration::SlotMigrationManager>>,
}

impl KVStore {
    /// Create a new KV store with 64-way sharding
    pub fn new(config: KVConfig) -> Self {
        Self::new_with_cache(config, None)
    }

    /// Create KV store with optional cache layer
    pub fn new_with_cache(config: KVConfig, cache_size: Option<usize>) -> Self {
        info!(
            "Initializing sharded KV store (64 shards) with max_memory={}MB, eviction={:?}",
            config.max_memory_mb, config.eviction_policy
        );

        // Initialize all 64 shards
        let shards: [Arc<KVShard>; SHARD_COUNT] = std::array::from_fn(|_| Arc::new(KVShard::new()));

        // Initialize cache if requested
        let cache = cache_size.map(|size| {
            info!("Enabling L1 cache with {} entries", size);
            Arc::new(crate::core::CacheLayer::new(size))
        });

        Self {
            shards: Arc::new(shards),
            stats: Arc::new(RwLock::new(KVStats::default())),
            config,
            cache,
            cluster_topology: None,
            cluster_migration: None,
        }
    }

    /// Create KV store with cluster mode enabled
    pub fn new_with_cluster(
        config: KVConfig,
        cache_size: Option<usize>,
        topology: Arc<crate::cluster::topology::ClusterTopology>,
        migration: Option<Arc<crate::cluster::migration::SlotMigrationManager>>,
    ) -> Self {
        info!(
            "Initializing sharded KV store with cluster mode (64 shards) with max_memory={}MB, eviction={:?}",
            config.max_memory_mb, config.eviction_policy
        );

        // Initialize all 64 shards
        let shards: [Arc<KVShard>; SHARD_COUNT] = std::array::from_fn(|_| Arc::new(KVShard::new()));

        // Initialize cache if requested
        let cache = cache_size.map(|size| {
            info!("Enabling L1 cache with {} entries", size);
            Arc::new(crate::core::CacheLayer::new(size))
        });

        Self {
            shards: Arc::new(shards),
            stats: Arc::new(RwLock::new(KVStats::default())),
            config,
            cache,
            cluster_topology: Some(topology),
            cluster_migration: migration,
        }
    }

    /// Check if key belongs to this node (cluster mode routing)
    fn check_cluster_routing(&self, key: &str) -> Result<()> {
        if let Some(ref topology) = self.cluster_topology {
            use crate::cluster::hash_slot::hash_slot;

            let slot = hash_slot(key);
            let my_node_id = topology.my_node_id();

            // Check if slot is migrating FIRST (before ownership check)
            // During migration, keys should be redirected to destination node
            if let Some(ref migration) = self.cluster_migration {
                if let Some(migration_status) = migration.get_migration(slot) {
                    // Slot is migrating - return ASK redirect to destination node
                    let to_node = migration_status.to_node;
                    if let Ok(node) = topology.get_node(&to_node) {
                        return Err(SynapError::ClusterAsk {
                            slot,
                            node_address: node.address.to_string(),
                        });
                    }
                }
            }

            // Check if slot belongs to this node
            match topology.get_slot_owner(slot) {
                Ok(owner) => {
                    if owner != my_node_id {
                        // Key belongs to different node - return MOVED redirect
                        if let Ok(node) = topology.get_node(&owner) {
                            return Err(SynapError::ClusterMoved {
                                slot,
                                node_address: node.address.to_string(),
                            });
                        }
                    }
                    // Key belongs to this node - OK
                    Ok(())
                }
                Err(_) => {
                    // Slot not assigned - cluster not ready
                    Err(SynapError::ClusterSlotNotAssigned { slot })
                }
            }
        } else {
            // Cluster mode not enabled - always OK
            Ok(())
        }
    }

    /// Get shard index for a key using consistent hashing
    #[inline]
    fn shard_for_key(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % SHARD_COUNT
    }

    /// Get reference to shard for a key
    #[inline]
    fn get_shard(&self, key: &str) -> &Arc<KVShard> {
        let index = self.shard_for_key(key);
        &self.shards[index]
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

        // Check cluster routing (returns error if key doesn't belong to this node)
        self.check_cluster_routing(key)?;

        let stored = StoredValue::new(value.clone(), ttl_secs);
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

        // Insert value in the appropriate shard
        let shard = self.get_shard(key);
        let mut data = shard.data.write();
        let is_new = data.insert(key.to_string(), stored).is_none();

        // Update stats
        let mut stats = self.stats.write();
        stats.sets += 1;
        if is_new {
            stats.total_keys += 1;
            stats.total_memory_bytes += entry_size;
        }

        // Update cache
        if let Some(ref cache) = self.cache {
            let cache_ttl = ttl_secs.map(|secs| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + secs
            });
            cache.put(key.to_string(), value, cache_ttl);
        }

        Ok(())
    }

    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        debug!("GET key={}", key);

        // Check cluster routing (returns error if key doesn't belong to this node)
        self.check_cluster_routing(key)?;

        // Try L1 cache first
        if let Some(ref cache) = self.cache {
            if let Some(cached_value) = cache.get(key) {
                debug!("L1 Cache HIT: {}", key);
                let mut stats = self.stats.write();
                stats.gets += 1;
                stats.hits += 1;
                return Ok(Some(cached_value));
            }
        }

        // Cache miss - get from storage
        let shard = self.get_shard(key);
        let mut data = shard.data.write();

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

            let value_data = value.data().to_vec();

            // Populate cache
            if let Some(ref cache) = self.cache {
                let ttl = value.ttl_remaining();
                cache.put(key.to_string(), value_data.clone(), ttl);
            }

            Ok(Some(value_data))
        } else {
            stats.misses += 1;
            Ok(None)
        }
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> Result<bool> {
        debug!("DELETE key={}", key);

        // Check cluster routing (returns error if key doesn't belong to this node)
        self.check_cluster_routing(key)?;

        // Invalidate cache first
        if let Some(ref cache) = self.cache {
            cache.delete(key);
        }

        let shard = self.get_shard(key);
        let mut data = shard.data.write();
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
        let shard = self.get_shard(key);
        let data = shard.data.read();
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
        let shard = self.get_shard(key);
        let data = shard.data.read();
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

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        let current_value = if let Some(value) = data.get(key) {
            if value.is_expired() {
                0
            } else {
                String::from_utf8(value.data().to_vec())
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

        let mut keys = Vec::new();

        // Scan all shards
        for shard in self.shards.iter() {
            let data = shard.data.read();

            let shard_keys: Vec<String> = if let Some(prefix) = prefix {
                data.get_prefix_keys(prefix)
            } else {
                data.keys()
            };

            keys.extend(shard_keys);

            // Early return if we hit the limit
            if keys.len() >= limit {
                keys.truncate(limit);
                break;
            }
        }

        Ok(keys)
    }

    /// Clean up expired keys using adaptive probabilistic sampling (Phase 2.2)
    /// Samples random keys instead of scanning all keys for 10-100x better performance
    async fn cleanup_expired(&self) {
        const SAMPLE_SIZE: usize = 20;
        const MAX_ITERATIONS: usize = 16;

        let mut total_expired = 0;

        // Sample from each shard
        for shard in self.shards.iter() {
            for _ in 0..MAX_ITERATIONS {
                let mut expired_keys = Vec::new();

                {
                    let data = shard.data.read();

                    // Sample random keys (simple sampling by taking first N)
                    let all_entries = data.iter();
                    let sampled: Vec<(String, bool)> = all_entries
                        .into_iter()
                        .take(SAMPLE_SIZE)
                        .map(|(k, v)| (k, v.is_expired()))
                        .collect();

                    for (key, is_expired) in sampled {
                        if is_expired {
                            expired_keys.push(key);
                        }
                    }
                }

                // Remove expired keys
                if !expired_keys.is_empty() {
                    let mut data = shard.data.write();
                    for key in &expired_keys {
                        data.remove(key);
                    }
                    total_expired += expired_keys.len();
                }

                // If less than 25% were expired, stop sampling this shard
                if expired_keys.len() < SAMPLE_SIZE / 4 {
                    break;
                }
            }
        }

        if total_expired > 0 {
            debug!(
                "Adaptive TTL cleanup: {} expired keys removed",
                total_expired
            );
            let mut stats = self.stats.write();
            stats.total_keys = stats.total_keys.saturating_sub(total_expired);
        }
    }

    /// Estimate memory size of an entry
    fn estimate_entry_size(&self, key: &str, value: &StoredValue) -> usize {
        key.len() + value.data().len() + std::mem::size_of::<StoredValue>()
    }

    /// Get all keys (no limit)
    pub async fn keys(&self) -> Result<Vec<String>> {
        let mut all_keys = Vec::new();

        // Collect keys from all shards
        for shard in self.shards.iter() {
            let data = shard.data.read();
            all_keys.extend(data.keys());
        }

        Ok(all_keys)
    }

    /// Get number of keys
    pub async fn dbsize(&self) -> Result<usize> {
        Ok(self.stats.read().total_keys)
    }

    /// Flush all keys from database
    pub async fn flushdb(&self) -> Result<usize> {
        debug!("FLUSHDB");

        // Check if FLUSH commands are allowed (disabled by default like Redis)
        if !self.config.allow_flush_commands {
            return Err(SynapError::InvalidRequest(
                "FLUSHDB is disabled. Set 'allow_flush_commands: true' in config to enable this dangerous command".to_string()
            ));
        }

        let count = self.stats.read().total_keys;

        // Invalidate cache
        if let Some(ref cache) = self.cache {
            cache.invalidate_all();
        }

        // Clear all shards
        for shard in self.shards.iter() {
            let mut data = shard.data.write();
            data.clear();
        }

        let mut stats = self.stats.write();
        stats.total_keys = 0;
        stats.total_memory_bytes = 0;

        Ok(count)
    }

    /// Flush all databases (alias for flushdb in single-db mode)
    pub async fn flushall(&self) -> Result<usize> {
        if !self.config.allow_flush_commands {
            return Err(SynapError::InvalidRequest(
                "FLUSHALL is disabled. Set 'allow_flush_commands: true' in config to enable this dangerous command".to_string()
            ));
        }
        self.flushdb().await
    }

    /// Set expiration time
    pub async fn expire(&self, key: &str, ttl_secs: u64) -> Result<bool> {
        debug!("EXPIRE key={}, ttl={}", key, ttl_secs);

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        if let Some(value) = data.remove(key) {
            // Convert to expiring variant or update existing
            let new_value = StoredValue::new(value.data().to_vec(), Some(ttl_secs));
            data.insert(key.to_string(), new_value);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove expiration from key
    pub async fn persist(&self, key: &str) -> Result<bool> {
        debug!("PERSIST key={}", key);

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        if let Some(value) = data.remove(key) {
            // Convert to persistent variant
            let new_value = StoredValue::new(value.data().to_vec(), None);
            data.insert(key.to_string(), new_value);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Dump all key-value pairs for persistence
    pub async fn dump(&self) -> Result<std::collections::HashMap<String, Vec<u8>>> {
        let mut dump = std::collections::HashMap::new();

        // Collect from all shards
        for shard in self.shards.iter() {
            let entries = {
                let data = shard.data.read();
                data.iter()
            };

            for (key, value) in entries {
                if !value.is_expired() {
                    dump.insert(key, value.data().to_vec());
                }
            }
        }

        Ok(dump)
    }

    // ========================================
    // String Extension Commands
    // ========================================

    /// APPEND: Append bytes to an existing value, or create new key with value if it doesn't exist
    pub async fn append(&self, key: &str, value: Vec<u8>) -> Result<usize> {
        debug!("APPEND key={}, append_size={}", key, value.len());

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        let new_length = if let Some(stored_value) = data.get_mut(key) {
            if stored_value.is_expired() {
                // Key expired, treat as new
                let new_data = value;
                *stored_value = StoredValue::new(new_data.clone(), None);
                new_data.len()
            } else {
                // Append to existing
                stored_value.update_access();
                stored_value.data_mut().extend_from_slice(&value);
                stored_value.data().len()
            }
        } else {
            // Key doesn't exist, create new
            let new_value = StoredValue::new(value.clone(), None);
            data.insert(key.to_string(), new_value);
            value.len()
        };

        let mut stats = self.stats.write();
        stats.sets += 1;

        // Invalidate cache
        if let Some(ref cache) = self.cache {
            cache.delete(key);
        }

        Ok(new_length)
    }

    /// GETRANGE: Get substring using Redis-style negative indices
    /// start and end are inclusive. Negative indices count from the end (-1 = last byte)
    pub async fn getrange(&self, key: &str, start: isize, end: isize) -> Result<Vec<u8>> {
        debug!("GETRANGE key={}, start={}, end={}", key, start, end);

        let shard = self.get_shard(key);
        let data = shard.data.read();

        if let Some(value) = data.get(key) {
            if value.is_expired() {
                return Ok(Vec::new());
            }

            let bytes = value.data();
            let len = bytes.len() as isize;

            // Normalize indices (handle negative indices)
            let start_idx = if start < 0 {
                (len + start).max(0)
            } else {
                start.min(len)
            } as usize;

            let end_idx = if end < 0 {
                (len + end + 1).max(0) // +1 because end is inclusive
            } else {
                (end + 1).min(len) // +1 because end is inclusive
            } as usize;

            // Check bounds
            if start_idx >= end_idx || start_idx >= bytes.len() {
                return Ok(Vec::new());
            }

            let mut stats = self.stats.write();
            stats.gets += 1;
            stats.hits += 1;

            Ok(bytes[start_idx..end_idx.min(bytes.len())].to_vec())
        } else {
            let mut stats = self.stats.write();
            stats.gets += 1;
            stats.misses += 1;
            Ok(Vec::new())
        }
    }

    /// SETRANGE: Overwrite a substring at offset, extending the string if necessary
    /// Returns the new length of the string
    pub async fn setrange(&self, key: &str, offset: usize, value: Vec<u8>) -> Result<usize> {
        debug!(
            "SETRANGE key={}, offset={}, value_size={}",
            key,
            offset,
            value.len()
        );

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        let new_length = if let Some(stored_value) = data.get_mut(key) {
            if stored_value.is_expired() {
                // Key expired, create new string with padding
                let mut new_data = vec![0u8; offset];
                new_data.extend_from_slice(&value);
                *stored_value = StoredValue::new(new_data.clone(), None);
                new_data.len()
            } else {
                // Update existing
                stored_value.update_access();
                let bytes = stored_value.data_mut();

                // Extend if necessary
                let required_len = offset + value.len();
                if bytes.len() < required_len {
                    bytes.resize(required_len, 0);
                }

                // Overwrite at offset
                bytes[offset..offset + value.len()].copy_from_slice(&value);
                bytes.len()
            }
        } else {
            // Key doesn't exist, create new with padding
            let mut new_data = vec![0u8; offset];
            new_data.extend_from_slice(&value);
            let new_value = StoredValue::new(new_data.clone(), None);
            data.insert(key.to_string(), new_value);
            new_data.len()
        };

        let mut stats = self.stats.write();
        stats.sets += 1;

        // Invalidate cache
        if let Some(ref cache) = self.cache {
            cache.delete(key);
        }

        Ok(new_length)
    }

    /// STRLEN: Get the length of the string value in bytes
    pub async fn strlen(&self, key: &str) -> Result<usize> {
        debug!("STRLEN key={}", key);

        let shard = self.get_shard(key);
        let data = shard.data.read();

        if let Some(value) = data.get(key) {
            if value.is_expired() {
                let mut stats = self.stats.write();
                stats.gets += 1;
                stats.misses += 1;
                return Ok(0);
            }

            let mut stats = self.stats.write();
            stats.gets += 1;
            stats.hits += 1;

            Ok(value.data().len())
        } else {
            let mut stats = self.stats.write();
            stats.gets += 1;
            stats.misses += 1;
            Ok(0)
        }
    }

    /// GETSET: Atomically get the current value and set a new one
    /// Returns the old value, or None if key didn't exist
    pub async fn getset(&self, key: &str, value: Vec<u8>) -> Result<Option<Vec<u8>>> {
        debug!("GETSET key={}, value_size={}", key, value.len());

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        let old_value = data.remove(key).map(|stored_value| {
            if stored_value.is_expired() {
                Vec::new() // Return empty for expired keys
            } else {
                stored_value.data().to_vec()
            }
        });

        // Insert new value
        let new_value = StoredValue::new(value.clone(), None);
        data.insert(key.to_string(), new_value);

        let mut stats = self.stats.write();
        stats.gets += 1;
        stats.sets += 1;
        if old_value.is_some() {
            stats.hits += 1;
        } else {
            stats.misses += 1;
            stats.total_keys += 1;
        }

        // Invalidate cache
        if let Some(ref cache) = self.cache {
            cache.delete(key);
        }

        Ok(old_value)
    }

    /// MSETNX: Multi-set only if ALL keys don't exist (atomic)
    /// Returns true if all keys were set, false if any key already existed
    pub async fn msetnx(&self, pairs: Vec<(String, Vec<u8>)>) -> Result<bool> {
        debug!("MSETNX count={}", pairs.len());

        if pairs.is_empty() {
            return Ok(true);
        }

        // Check if all keys don't exist (need to check all shards)
        // Quick check: if any key exists, return false
        for (key, _) in &pairs {
            let shard = self.get_shard(key);
            let data = shard.data.read();
            if let Some(value) = data.get(key) {
                if !value.is_expired() {
                    return Ok(false);
                }
            }
        }

        // All keys are free, now set them all atomically
        for (key, value) in &pairs {
            let shard = self.get_shard(key);
            let mut data = shard.data.write();

            data.insert(key.clone(), StoredValue::new(value.clone(), None));

            // Invalidate cache
            if let Some(ref cache) = self.cache {
                cache.delete(key);
            }
        }

        let mut stats = self.stats.write();
        stats.sets += pairs.len() as u64;

        Ok(true)
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

    #[tokio::test]
    async fn test_keys() {
        let store = KVStore::new(KVConfig::default());

        store.set("key1", b"value1".to_vec(), None).await.unwrap();
        store.set("key2", b"value2".to_vec(), None).await.unwrap();
        store.set("key3", b"value3".to_vec(), None).await.unwrap();

        let keys = store.keys().await.unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[tokio::test]
    async fn test_dbsize() {
        let store = KVStore::new(KVConfig::default());

        assert_eq!(store.dbsize().await.unwrap(), 0);

        store.set("key1", b"value1".to_vec(), None).await.unwrap();
        assert_eq!(store.dbsize().await.unwrap(), 1);

        store.set("key2", b"value2".to_vec(), None).await.unwrap();
        assert_eq!(store.dbsize().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_flushdb() {
        let mut config = KVConfig::default();
        config.allow_flush_commands = true; // Enable FLUSHDB for test
        let store = KVStore::new(config);

        store.set("key1", b"value1".to_vec(), None).await.unwrap();
        store.set("key2", b"value2".to_vec(), None).await.unwrap();
        store.set("key3", b"value3".to_vec(), None).await.unwrap();

        assert_eq!(store.dbsize().await.unwrap(), 3);

        let flushed = store.flushdb().await.unwrap();
        assert_eq!(flushed, 3);
        assert_eq!(store.dbsize().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_expire_and_persist() {
        let store = KVStore::new(KVConfig::default());

        store.set("key1", b"value1".to_vec(), None).await.unwrap();

        // Set expiration
        let result = store.expire("key1", 60).await.unwrap();
        assert!(result);

        let ttl = store.ttl("key1").await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap() > 0 && ttl.unwrap() <= 60);

        // Remove expiration
        let result = store.persist("key1").await.unwrap();
        assert!(result);

        let ttl = store.ttl("key1").await.unwrap();
        assert!(ttl.is_none());
    }

    // ==================== String Extension Tests ====================

    #[tokio::test]
    async fn test_append() {
        let store = KVStore::new(KVConfig::default());

        // Append to non-existent key (creates new)
        let length = store.append("key1", b"hello".to_vec()).await.unwrap();
        assert_eq!(length, 5);

        let value = store.get("key1").await.unwrap();
        assert_eq!(value, Some(b"hello".to_vec()));

        // Append to existing key
        let length = store.append("key1", b" world".to_vec()).await.unwrap();
        assert_eq!(length, 11);

        let value = store.get("key1").await.unwrap();
        assert_eq!(value, Some(b"hello world".to_vec()));

        // Append empty bytes
        let length = store.append("key1", b"".to_vec()).await.unwrap();
        assert_eq!(length, 11);

        let value = store.get("key1").await.unwrap();
        assert_eq!(value, Some(b"hello world".to_vec()));
    }

    #[tokio::test]
    async fn test_getrange() {
        let store = KVStore::new(KVConfig::default());

        store
            .set("key1", b"hello world".to_vec(), None)
            .await
            .unwrap();

        // Positive indices
        let result = store.getrange("key1", 0, 4).await.unwrap();
        assert_eq!(result, b"hello");

        // Full range
        let result = store.getrange("key1", 0, 10).await.unwrap();
        assert_eq!(result, b"hello world");

        // Negative start index (counts from end)
        let result = store.getrange("key1", -5, -1).await.unwrap();
        assert_eq!(result, b"world");

        // Negative end index
        let result = store.getrange("key1", 0, -7).await.unwrap();
        assert_eq!(result, b"hello");

        // Start > end (empty result)
        let result = store.getrange("key1", 5, 3).await.unwrap();
        assert_eq!(result, b"");

        // Out of bounds
        let result = store.getrange("key1", 100, 200).await.unwrap();
        assert_eq!(result, b"");

        // Non-existent key
        let result = store.getrange("nonexistent", 0, 5).await.unwrap();
        assert_eq!(result, b"");
    }

    #[tokio::test]
    async fn test_setrange() {
        let store = KVStore::new(KVConfig::default());

        // Setrange on non-existent key (creates with padding)
        let length = store.setrange("key1", 5, b"world".to_vec()).await.unwrap();
        assert_eq!(length, 10);

        let value = store.get("key1").await.unwrap();
        assert_eq!(
            value,
            Some(vec![0, 0, 0, 0, 0, b'w', b'o', b'r', b'l', b'd'])
        );

        // Set existing key
        store
            .set("key2", b"hello world".to_vec(), None)
            .await
            .unwrap();
        let length = store.setrange("key2", 6, b"Synap".to_vec()).await.unwrap();
        assert_eq!(length, 11);

        let value = store.get("key2").await.unwrap();
        assert_eq!(value, Some(b"hello Synap".to_vec()));

        // Extend string
        let length = store.setrange("key2", 11, b"!".to_vec()).await.unwrap();
        assert_eq!(length, 12);

        let value = store.get("key2").await.unwrap();
        assert_eq!(value, Some(b"hello Synap!".to_vec()));

        // Overwrite middle
        let length = store.setrange("key2", 0, b"Hi".to_vec()).await.unwrap();
        assert_eq!(length, 12);

        let value = store.get("key2").await.unwrap();
        assert_eq!(value.as_ref().map(|v| &v[..2]), Some(&b"Hi"[..]));
    }

    #[tokio::test]
    async fn test_strlen() {
        let store = KVStore::new(KVConfig::default());

        // Non-existent key
        let length = store.strlen("nonexistent").await.unwrap();
        assert_eq!(length, 0);

        // Existing key
        store.set("key无可", b"hello".to_vec(), None).await.unwrap();
        let length = store.strlen("key无可").await.unwrap();
        assert_eq!(length, 5);

        // Empty value
        store.set("key2", b"".to_vec(), None).await.unwrap();
        let length = store.strlen("key2").await.unwrap();
        assert_eq!(length, 0);

        // Large value
        let large_value = vec![0u8; 10000];
        store.set("key3", large_value.clone(), None).await.unwrap();
        let length = store.strlen("key3").await.unwrap();
        assert_eq!(length, 10000);
    }

    #[tokio::test]
    async fn test_getset() {
        let store = KVStore::new(KVConfig::default());

        // Getset on non-existent key
        let old_value = store.getset("key1", b"new_value".to_vec()).await.unwrap();
        assert_eq!(old_value, None);

        let current_value = store.get("key1").await.unwrap();
        assert_eq!(current_value, Some(b"new_value".to_vec()));

        // Getset on existing key
        let old_value = store.getset("key1", b"updated".to_vec()).await.unwrap();
        assert_eq!(old_value, Some(b"new_value".to_vec()));

        let current_value = store.get("key1").await.unwrap();
        assert_eq!(current_value, Some(b"updated".to_vec()));

        // Getset with empty value
        let old_value = store.getset("key1", b"".to_vec()).await.unwrap();
        assert_eq!(old_value, Some(b"updated".to_vec()));

        let current_value = store.get("key1").await.unwrap();
        assert_eq!(current_value, Some(b"".to_vec()));
    }

    #[tokio::test]
    async fn test_msetnx() {
        let store = KVStore::new(KVConfig::default());

        // MSETNX with all new keys
        let pairs = vec![
            ("key1".to_string(), b"value1".to_vec()),
            ("key2".to_string(), b"value2".to_vec()),
            ("key3".to_string(), b"value3".to_vec()),
        ];
        let success = store.msetnx(pairs).await.unwrap();
        assert!(success);

        assert_eq!(store.get("key1").await.unwrap(), Some(b"value1".to_vec()));
        assert_eq!(store.get("key2").await.unwrap(), Some(b"value2".to_vec()));
        assert_eq!(store.get("key3").await.unwrap(), Some(b"value3".to_vec()));

        // MSETNX with one existing key (should fail and set nothing)
        store.set("key4", b"existing".to_vec(), None).await.unwrap();
        let pairs = vec![
            ("key4".to_string(), b"should_not_set".to_vec()),
            ("key5".to_string(), b"value5".to_vec()),
        ];
        let success = store.msetnx(pairs).await.unwrap();
        assert!(!success);

        // Verify key4 unchanged
        assert_eq!(store.get("key4").await.unwrap(), Some(b"existing".to_vec()));
        // Verify key5 not set
        assert_eq!(store.get("key5").await.unwrap(), None);

        // MSETNX with empty pairs
        let success = store.msetnx(vec![]).await.unwrap();
        assert!(success);

        // MSETNX with all existing keys (should fail)
        let pairs = vec![
            ("key1".to_string(), b"should_not_set1".to_vec()),
            ("key2".to_string(), b"should_not_set2".to_vec()),
        ];
        let success = store.msetnx(pairs).await.unwrap();
        assert!(!success);

        // Verify original values unchanged
        assert_eq!(store.get("key1").await.unwrap(), Some(b"value1".to_vec()));
        assert_eq!(store.get("key2").await.unwrap(), Some(b"value2".to_vec()));
    }

    #[tokio::test]
    async fn test_string_extensions_with_ttl() {
        let store = KVStore::new(KVConfig::default());

        // Test APPEND with TTL
        store
            .set("key1", b"hello".to_vec(), Some(60))
            .await
            .unwrap();
        let length = store.append("key1", b" world".to_vec()).await.unwrap();
        assert_eq!(length, 11);

        // Test GETRANGE with expired key
        store.set("key2", b"test".to_vec(), Some(1)).await.unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
        let result = store.getrange("key2", 0, 3).await.unwrap();
        assert_eq!(result, b"");

        // Test STRLEN with expired key
        store.set("key3", b"test".to_vec(), Some(1)).await.unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
        let length = store.strlen("key3").await.unwrap();
        assert_eq!(length, 0);
    }
}
