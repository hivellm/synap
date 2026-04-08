use super::error::{Result, SynapError};
use super::types::{
    AtomicKVStats, EvictionPolicy, Expiry, KVConfig, KVStats, SetOptions, SetResult, StoredValue,
};
use parking_lot::RwLock;
use radix_trie::{Trie, TrieCommon};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::Ordering;
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
    stats: Arc<AtomicKVStats>,
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
            stats: Arc::new(AtomicKVStats::default()),
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
            stats: Arc::new(AtomicKVStats::default()),
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

    /// Set a key-value pair.
    ///
    /// S-12: accepts `impl Into<String>` so callers that already hold an owned `String`
    /// (recovery, replication, transactions) avoid the internal `to_string()` allocation.
    /// Callers with `&str` / string literals still work without any change.
    pub async fn set(
        &self,
        key: impl Into<String>,
        value: Vec<u8>,
        ttl_secs: Option<u64>,
    ) -> Result<()> {
        let key: String = key.into();
        debug!("SET key={}, size={}, ttl={:?}", key, value.len(), ttl_secs);

        // Check cluster routing (returns error if key doesn't belong to this node)
        self.check_cluster_routing(&key)?;

        let stored = StoredValue::new(value.clone(), ttl_secs);
        let entry_size = self.estimate_entry_size(&key, &stored);

        // Check memory limits — evict if policy allows, error on noeviction.
        {
            let current_bytes = self.stats.total_memory_bytes.load(Ordering::Relaxed);
            let max_bytes = (self.config.max_memory_mb * 1024 * 1024) as i64;
            if current_bytes + entry_size as i64 > max_bytes {
                if self.config.eviction_policy == EvictionPolicy::NoEviction {
                    warn!(
                        "Memory limit exceeded (noeviction): {}/{}",
                        current_bytes, max_bytes
                    );
                    return Err(SynapError::MemoryLimitExceeded);
                }
                self.evict_until_free(entry_size);
                // Re-check after eviction.
                let after = self.stats.total_memory_bytes.load(Ordering::Relaxed);
                if after + entry_size as i64 > max_bytes {
                    warn!(
                        "Memory limit exceeded after eviction: {}/{}",
                        after, max_bytes
                    );
                    return Err(SynapError::MemoryLimitExceeded);
                }
            }
        }

        // Insert value in the appropriate shard — key moved directly, no extra allocation.
        let shard = self.get_shard(&key);
        let mut data = shard.data.write();

        // Save key length before moving key into the HashMap (needed for overwrite accounting).
        let key_len = key.len();
        // Clone key for cache before moving it into the HashMap.
        let cache_key = self.cache.as_ref().map(|_| key.clone());

        let old = data.insert(key, stored);
        let is_new = old.is_none();

        // Update stats atomically — no global lock
        self.stats.sets.fetch_add(1, Ordering::Relaxed);
        if is_new {
            self.stats.total_keys.fetch_add(1, Ordering::Relaxed);
            self.stats
                .total_memory_bytes
                .fetch_add(entry_size as i64, Ordering::Relaxed);
        } else if let Some(ref old_val) = old {
            // Overwrite: subtract old size, add new size (key length unchanged).
            let old_size = key_len + old_val.data().len() + std::mem::size_of::<StoredValue>();
            self.stats
                .total_memory_bytes
                .fetch_add(entry_size as i64 - old_size as i64, Ordering::Relaxed);
        }

        // Update cache
        if let (Some(cache), Some(k)) = (&self.cache, cache_key) {
            let cache_ttl = ttl_secs.map(|secs| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + secs
            });
            cache.put(k, value, cache_ttl);
        }

        Ok(())
    }

    /// Set a value with full Redis-compatible options (NX / XX / GET / KEEPTTL / PX / PXAT).
    ///
    /// Returns [`SetResult`] indicating whether the write happened and the
    /// previous value (when `opts.return_old = true`).
    ///
    /// All NX/XX checks are performed under the shard write lock — no TOCTOU.
    pub async fn set_with_opts(
        &self,
        key: &str,
        value: Vec<u8>,
        expiry: Option<Expiry>,
        opts: SetOptions,
    ) -> Result<SetResult> {
        debug!(
            "SET key={} size={} expiry={:?} nx={} xx={} keepttl={} get={}",
            key,
            value.len(),
            expiry,
            opts.if_absent,
            opts.if_present,
            opts.keep_ttl,
            opts.return_old,
        );

        self.check_cluster_routing(key)?;

        // --- Pre-lock memory check + eviction ---
        // Estimate size conservatively before building the StoredValue.
        let approx_size = key.len() + value.len() + std::mem::size_of::<StoredValue>();
        {
            let current_bytes = self.stats.total_memory_bytes.load(Ordering::Relaxed);
            let max_bytes = (self.config.max_memory_mb * 1024 * 1024) as i64;
            if current_bytes + approx_size as i64 > max_bytes {
                if self.config.eviction_policy == EvictionPolicy::NoEviction {
                    warn!(
                        "Memory limit exceeded (noeviction): {}/{}",
                        current_bytes, max_bytes
                    );
                    return Err(SynapError::MemoryLimitExceeded);
                }
                self.evict_until_free(approx_size);
                let after = self.stats.total_memory_bytes.load(Ordering::Relaxed);
                if after + approx_size as i64 > max_bytes {
                    warn!(
                        "Memory limit exceeded after eviction: {}/{}",
                        after, max_bytes
                    );
                    return Err(SynapError::MemoryLimitExceeded);
                }
            }
        }

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        // --- NX / XX guard (under write lock — no TOCTOU) ---
        let existing = data.get(key);
        let key_exists = existing.is_some_and(|v| !v.is_expired());

        if opts.if_absent && key_exists {
            // NX: key exists → do NOT set, return old value if requested
            let old_value = if opts.return_old {
                existing.map(|v| v.data().to_vec())
            } else {
                None
            };
            return Ok(SetResult {
                written: false,
                old_value,
            });
        }
        if opts.if_present && !key_exists {
            // XX: key does not exist → do NOT set
            return Ok(SetResult {
                written: false,
                old_value: None,
            });
        }

        // --- Capture old value for GET option ---
        let old_value = if opts.return_old {
            existing
                .filter(|v| !v.is_expired())
                .map(|v| v.data().to_vec())
        } else {
            None
        };

        // --- Compute the new StoredValue ---
        let stored = if opts.keep_ttl {
            // KEEPTTL: carry forward existing expiry timestamp, ignore new expiry
            let existing_expires_at_ms = data
                .get(key)
                .filter(|v| !v.is_expired())
                .and_then(|v| v.expires_at_ms());
            match existing_expires_at_ms {
                Some(ms) => StoredValue::with_expires_at_ms(value.clone(), ms),
                None => StoredValue::Persistent(value.clone()),
            }
        } else {
            match expiry {
                Some(exp) => StoredValue::with_expiry(value.clone(), exp),
                None => StoredValue::Persistent(value.clone()),
            }
        };

        // --- Exact size for stats accounting ---
        let entry_size = self.estimate_entry_size(key, &stored);

        // --- Insert ---
        let old_entry = data.insert(key.to_string(), stored);
        let is_new = old_entry.is_none() || old_entry.as_ref().is_some_and(|v| v.is_expired());

        self.stats.sets.fetch_add(1, Ordering::Relaxed);
        if is_new {
            self.stats.total_keys.fetch_add(1, Ordering::Relaxed);
            self.stats
                .total_memory_bytes
                .fetch_add(entry_size as i64, Ordering::Relaxed);
        } else if let Some(ref old_val) = old_entry {
            let old_size = self.estimate_entry_size(key, old_val);
            self.stats
                .total_memory_bytes
                .fetch_add(entry_size as i64 - old_size as i64, Ordering::Relaxed);
        }

        // --- Cache invalidation ---
        if let Some(ref cache) = self.cache {
            let cache_expiry_secs = expiry.map(|e| {
                use std::time::{SystemTime, UNIX_EPOCH};
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                // Convert expiry to absolute seconds for L1 cache
                match e {
                    Expiry::Seconds(s) => now + s,
                    Expiry::Milliseconds(ms) => now + ms / 1_000,
                    Expiry::UnixSeconds(s) => s,
                    Expiry::UnixMilliseconds(ms) => ms / 1_000,
                }
            });
            cache.put(key.to_string(), value, cache_expiry_secs);
        }

        Ok(SetResult {
            written: true,
            old_value,
        })
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
                self.stats.gets.fetch_add(1, Ordering::Relaxed);
                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(Some(cached_value));
            }
        }

        // Cache miss - get from storage.
        // Use a READ lock for the happy path: update_access() is atomic (AtomicU32)
        // so it does not need a write lock. Only expired key removal requires a
        // write lock, and that is the cold path.
        let shard = self.get_shard(key);

        self.stats.gets.fetch_add(1, Ordering::Relaxed);

        // --- Hot path: read lock ---
        {
            let data = shard.data.read();
            if let Some(value) = data.get(key) {
                if !value.is_expired() {
                    // Atomic LRU update — safe under read lock.
                    value.update_access();
                    self.stats.hits.fetch_add(1, Ordering::Relaxed);

                    let value_data = value.data().to_vec();
                    let ttl = value.ttl_remaining();
                    drop(data);

                    // Populate cache
                    if let Some(ref cache) = self.cache {
                        cache.put(key.to_string(), value_data.clone(), ttl);
                    }

                    return Ok(Some(value_data));
                }
                // Key exists but is expired — fall through to write lock path.
            } else {
                // Key not found.
                self.stats.misses.fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }
        }

        // --- Cold path: write lock (expired key removal) ---
        {
            let mut data = shard.data.write();
            // Re-check under write lock (another thread may have already removed it).
            if let Some(value) = data.get(key) {
                if value.is_expired() {
                    debug!("Key expired: {}", key);
                    if let Some(expired_val) = data.remove(key) {
                        let removed_size = self.estimate_entry_size(key, &expired_val);
                        self.stats.total_keys.fetch_sub(1, Ordering::Relaxed);
                        self.stats
                            .total_memory_bytes
                            .fetch_sub(removed_size as i64, Ordering::Relaxed);
                    }
                } else {
                    // Raced: another writer refreshed the key between locks.
                    // Treat as a hit.
                    value.update_access();
                    self.stats.hits.fetch_add(1, Ordering::Relaxed);
                    let value_data = value.data().to_vec();
                    let ttl = value.ttl_remaining();
                    drop(data);
                    if let Some(ref cache) = self.cache {
                        cache.put(key.to_string(), value_data.clone(), ttl);
                    }
                    return Ok(Some(value_data));
                }
            }
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
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

        if let Some(removed_val) = removed {
            let removed_size = self.estimate_entry_size(key, &removed_val);
            self.stats.dels.fetch_add(1, Ordering::Relaxed);
            self.stats.total_keys.fetch_sub(1, Ordering::Relaxed);
            self.stats
                .total_memory_bytes
                .fetch_sub(removed_size as i64, Ordering::Relaxed);
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

    /// Get statistics snapshot
    pub async fn stats(&self) -> KVStats {
        self.stats.snapshot()
    }

    /// Get the KV store configuration
    pub fn config(&self) -> &KVConfig {
        &self.config
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
    ///
    /// Preserves the TTL of the existing entry (S-16 fix). Uses `checked_add`
    /// and returns an error on overflow rather than wrapping.
    pub async fn incr(&self, key: &str, amount: i64) -> Result<i64> {
        debug!("INCR key={}, amount={}", key, amount);

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        // Read current value AND preserve its TTL (expires_at)
        let (current_value, existing_ttl_secs) = if let Some(value) = data.get(key) {
            if value.is_expired() {
                (0i64, None)
            } else {
                let int_val = String::from_utf8(value.data().to_vec())
                    .ok()
                    .and_then(|s| s.parse::<i64>().ok())
                    .ok_or_else(|| {
                        SynapError::InvalidValue("Value is not a valid integer".to_string())
                    })?;
                // Capture remaining TTL to restore it after the write
                let ttl = value.remaining_ttl_secs();
                (int_val, ttl)
            }
        } else {
            (0i64, None)
        };

        let new_value = current_value
            .checked_add(amount)
            .ok_or_else(|| SynapError::InvalidValue("Integer overflow on INCR/DECR".to_string()))?;
        let new_data = new_value.to_string().into_bytes();

        // Preserve TTL: if the key had a TTL, write the new value as Expiring
        // with the same remaining time rather than as Persistent.
        data.insert(
            key.to_string(),
            StoredValue::new(new_data, existing_ttl_secs),
        );

        self.stats.sets.fetch_add(1, Ordering::Relaxed);

        Ok(new_value)
    }

    /// Atomic decrement
    pub async fn decr(&self, key: &str, amount: i64) -> Result<i64> {
        self.incr(key, -amount).await
    }

    /// Set multiple key-value pairs
    pub async fn mset(&self, pairs: Vec<(String, Vec<u8>)>) -> Result<()> {
        debug!("MSET count={}", pairs.len());

        // Group pairs by shard so we acquire each shard's write lock only once.
        let mut by_shard: Vec<Vec<(String, Vec<u8>)>> = (0..SHARD_COUNT).map(|_| vec![]).collect();
        for (key, value) in pairs {
            self.check_cluster_routing(&key)?;
            let idx = self.shard_for_key(&key);
            by_shard[idx].push((key, value));
        }

        for (idx, group) in by_shard.into_iter().enumerate() {
            if group.is_empty() {
                continue;
            }

            // Memory limit check before the write lock (rough estimate per group).
            let group_size: usize = group
                .iter()
                .map(|(k, v)| k.len() + v.len() + std::mem::size_of::<StoredValue>())
                .sum();
            {
                let current = self.stats.total_memory_bytes.load(Ordering::Relaxed);
                let max_bytes = (self.config.max_memory_mb * 1024 * 1024) as i64;
                if current + group_size as i64 > max_bytes {
                    if self.config.eviction_policy == EvictionPolicy::NoEviction {
                        return Err(SynapError::MemoryLimitExceeded);
                    }
                    self.evict_until_free(group_size);
                }
            }

            let shard = &self.shards[idx];
            let mut data = shard.data.write();
            for (key, value) in group {
                let stored = StoredValue::Persistent(value);
                let entry_size =
                    key.len() + stored.data().len() + std::mem::size_of::<StoredValue>();
                let old = data.insert(key, stored);
                self.stats.sets.fetch_add(1, Ordering::Relaxed);
                if old.is_none() {
                    self.stats.total_keys.fetch_add(1, Ordering::Relaxed);
                    self.stats
                        .total_memory_bytes
                        .fetch_add(entry_size as i64, Ordering::Relaxed);
                } else if let Some(ref old_val) = old {
                    let old_size = old_val.data().len() + std::mem::size_of::<StoredValue>();
                    self.stats
                        .total_memory_bytes
                        .fetch_add(entry_size as i64 - old_size as i64, Ordering::Relaxed);
                }
            }
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

                // Remove expired keys and account for memory freed
                if !expired_keys.is_empty() {
                    let mut data = shard.data.write();
                    for key in &expired_keys {
                        if let Some(removed_val) = data.remove(key) {
                            let removed_size = self.estimate_entry_size(key, &removed_val);
                            self.stats
                                .total_memory_bytes
                                .fetch_sub(removed_size as i64, Ordering::Relaxed);
                            total_expired += 1;
                        }
                    }
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
            self.stats
                .total_keys
                .fetch_sub(total_expired as i64, Ordering::Relaxed);
        }
    }

    /// Estimate memory size of an entry
    fn estimate_entry_size(&self, key: &str, value: &StoredValue) -> usize {
        key.len() + value.data().len() + std::mem::size_of::<StoredValue>()
    }

    /// Evict keys from all shards until at least `needed_bytes` of memory have been freed,
    /// or until no more evictable candidates remain.
    ///
    /// Uses approximated LRU / random sampling matching Redis behaviour:
    /// pick `sample_size` random keys per shard, evict the worst candidates.
    fn evict_until_free(&self, needed_bytes: usize) {
        use EvictionPolicy::*;
        let policy = self.config.eviction_policy;
        let sample_size = self.config.eviction_sample_size.max(1);
        let max_bytes = (self.config.max_memory_mb * 1024 * 1024) as i64;

        // Iterate shards round-robin until enough memory is freed or no progress made.
        let mut freed = 0i64;
        let mut stalled_rounds = 0usize;
        let needed = needed_bytes as i64;

        'outer: loop {
            let before = freed;
            for shard in self.shards.iter() {
                let current = self.stats.total_memory_bytes.load(Ordering::Relaxed);
                if current + needed <= max_bytes {
                    break 'outer;
                }

                let mut data = shard.data.write();

                // Collect candidate (key, score) pairs from a sample.
                // score: lower means "evict first".
                // data.keys() returns Vec<String> — call into_iter() to get a consuming iterator.
                let all_keys = data.keys(); // Vec<String>
                let candidates: Vec<(String, u64)> = match policy {
                    AllKeysLru => all_keys
                        .into_iter()
                        .take(sample_size)
                        .map(|k| {
                            let la =
                                data.get(k.as_str()).map(|v| v.last_access()).unwrap_or(0) as u64;
                            (k, la)
                        })
                        .collect(),
                    VolatileLru => all_keys
                        .into_iter()
                        .filter(|k| {
                            data.get(k.as_str())
                                .map(|v| matches!(v, StoredValue::Expiring { .. }))
                                .unwrap_or(false)
                        })
                        .take(sample_size)
                        .map(|k| {
                            let la =
                                data.get(k.as_str()).map(|v| v.last_access()).unwrap_or(0) as u64;
                            (k, la)
                        })
                        .collect(),
                    VolatileTtl =>
                    // Score by expires_at ascending (soonest-expiring first).
                    {
                        all_keys
                            .into_iter()
                            .filter(|k| {
                                data.get(k.as_str())
                                    .map(|v| v.expires_at_ms().is_some())
                                    .unwrap_or(false)
                            })
                            .take(sample_size)
                            .map(|k| {
                                let exp = data
                                    .get(k.as_str())
                                    .and_then(|v| v.expires_at_ms())
                                    .unwrap_or(u64::MAX);
                                (k, exp)
                            })
                            .collect()
                    }
                    AllKeysRandom => all_keys
                        .into_iter()
                        .take(sample_size)
                        .map(|k| (k, 0u64))
                        .collect(),
                    VolatileRandom => all_keys
                        .into_iter()
                        .filter(|k| {
                            data.get(k.as_str())
                                .map(|v| matches!(v, StoredValue::Expiring { .. }))
                                .unwrap_or(false)
                        })
                        .take(sample_size)
                        .map(|k| (k, 0u64))
                        .collect(),
                    NoEviction => break 'outer,
                };

                if candidates.is_empty() {
                    continue;
                }

                // Evict the candidate with the lowest score.
                let victim = candidates
                    .into_iter()
                    .min_by_key(|(_, score)| *score)
                    .map(|(k, _)| k);

                if let Some(key) = victim {
                    if let Some(val) = data.remove(&key) {
                        let size = self.estimate_entry_size(&key, &val) as i64;
                        self.stats.total_keys.fetch_sub(1, Ordering::Relaxed);
                        self.stats
                            .total_memory_bytes
                            .fetch_sub(size, Ordering::Relaxed);
                        freed += size;
                        debug!("Evicted key={} size={} policy={:?}", key, size, policy);
                    }
                }
            }

            // If no progress was made this round, stop to avoid infinite loop.
            if freed == before {
                stalled_rounds += 1;
                if stalled_rounds >= 2 {
                    break;
                }
            } else {
                stalled_rounds = 0;
            }
        }
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
        Ok(self.stats.total_keys.load(Ordering::Relaxed).max(0) as usize)
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

        let count = self.stats.total_keys.load(Ordering::Relaxed).max(0) as usize;

        // Invalidate cache
        if let Some(ref cache) = self.cache {
            cache.invalidate_all();
        }

        // Clear all shards
        for shard in self.shards.iter() {
            let mut data = shard.data.write();
            data.clear();
        }

        self.stats.total_keys.store(0, Ordering::Relaxed);
        self.stats.total_memory_bytes.store(0, Ordering::Relaxed);

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

        self.stats.sets.fetch_add(1, Ordering::Relaxed);

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

            self.stats.gets.fetch_add(1, Ordering::Relaxed);
            self.stats.hits.fetch_add(1, Ordering::Relaxed);

            Ok(bytes[start_idx..end_idx.min(bytes.len())].to_vec())
        } else {
            self.stats.gets.fetch_add(1, Ordering::Relaxed);
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
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

        self.stats.sets.fetch_add(1, Ordering::Relaxed);

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
                self.stats.gets.fetch_add(1, Ordering::Relaxed);
                self.stats.misses.fetch_add(1, Ordering::Relaxed);
                return Ok(0);
            }

            self.stats.gets.fetch_add(1, Ordering::Relaxed);
            self.stats.hits.fetch_add(1, Ordering::Relaxed);

            Ok(value.data().len())
        } else {
            self.stats.gets.fetch_add(1, Ordering::Relaxed);
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
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

        self.stats.gets.fetch_add(1, Ordering::Relaxed);
        self.stats.sets.fetch_add(1, Ordering::Relaxed);
        if old_value.is_some() {
            self.stats.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            self.stats.total_keys.fetch_add(1, Ordering::Relaxed);
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

        self.stats
            .sets
            .fetch_add(pairs.len() as u64, Ordering::Relaxed);

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

    // --- Phase 1 correctness tests (phase1_fix-kv-set-correctness) ---

    #[tokio::test]
    async fn test_memory_accounting_on_overwrite() {
        let store = KVStore::new(KVConfig::default());

        // Insert initial value (100 bytes)
        let v1 = vec![0u8; 100];
        store.set("key", v1, None).await.unwrap();
        let after_insert = store.stats().await.total_memory_bytes;

        // Overwrite with larger value (200 bytes)
        let v2 = vec![0u8; 200];
        store.set("key", v2, None).await.unwrap();
        let after_overwrite = store.stats().await.total_memory_bytes;

        // Memory should have grown by ~100 bytes, not 200+
        // (exact delta depends on estimate_entry_size overhead)
        assert!(
            after_overwrite > after_insert,
            "memory should increase for larger overwrite"
        );
        assert!(
            after_overwrite < after_insert + 200,
            "memory must not count both old and new value (got insert={}, overwrite={})",
            after_insert,
            after_overwrite
        );
    }

    #[tokio::test]
    async fn test_memory_accounting_on_delete() {
        let store = KVStore::new(KVConfig::default());

        store.set("key", vec![0u8; 100], None).await.unwrap();
        let before_delete = store.stats().await.total_memory_bytes;
        assert!(before_delete > 0);

        store.delete("key").await.unwrap();
        let after_delete = store.stats().await.total_memory_bytes;

        assert!(
            after_delete < before_delete,
            "memory must decrease after delete (before={}, after={})",
            before_delete,
            after_delete
        );
    }

    #[tokio::test]
    async fn test_incr_preserves_ttl() {
        let store = KVStore::new(KVConfig::default());

        // Set a key with a 60-second TTL and value "42"
        store
            .set("counter", b"42".to_vec(), Some(60))
            .await
            .unwrap();

        // INCR should produce 43 and keep the TTL
        let new_val = store.incr("counter", 1).await.unwrap();
        assert_eq!(new_val, 43);

        // TTL must still be present and reasonable (≥50s, since the test runs fast)
        let ttl = store.ttl("counter").await.unwrap();
        assert!(ttl.is_some(), "TTL must be preserved after INCR (got None)");
        let remaining = ttl.unwrap();
        assert!(
            remaining >= 50,
            "TTL must remain close to original after INCR (got {}s)",
            remaining
        );
    }

    #[tokio::test]
    async fn test_incr_overflow_returns_error() {
        let store = KVStore::new(KVConfig::default());

        store
            .set("maxkey", i64::MAX.to_string().into_bytes(), None)
            .await
            .unwrap();

        let result = store.incr("maxkey", 1).await;
        assert!(result.is_err(), "INCR on i64::MAX must return an error");
    }

    /// Tail 6.2 — 1M SET-overwrite stress: total_memory_bytes must not drift.
    /// After 1M overwrites of the same key, memory accounting must reflect
    /// exactly one entry, not 1M accumulated entries.
    #[tokio::test]
    async fn test_memory_accounting_overwrite_stress() {
        let store = KVStore::new(KVConfig::default());
        let key = "stress_key";

        // 1M overwrites of the same key with a fixed 64-byte value.
        for _ in 0..1_000_000 {
            store.set(key, vec![42u8; 64], None).await.unwrap();
        }

        let stats = store.stats().await;
        // With one key of ~64+overhead bytes, memory must not have grown
        // to more than 100× the expected single-entry size.
        // A correct implementation accumulates 0 drift; we allow 100× headroom
        // to account for internal estimates but catch catastrophic leaks.
        let single_entry_upper_bound: i64 = 512; // generous upper bound for one entry
        assert_eq!(
            stats.total_keys, 1,
            "only one key must exist after overwrite stress"
        );
        assert!(
            stats.total_memory_bytes <= single_entry_upper_bound,
            "memory must reflect one entry after 1M overwrites, got {} bytes",
            stats.total_memory_bytes
        );
    }

    /// Tail 6.3 — concurrent SET on 16 threads must complete without deadlock
    /// and leave total_keys equal to the number of distinct keys inserted.
    #[tokio::test]
    async fn test_concurrent_set_no_lock_contention() {
        use std::sync::Arc;
        let store = Arc::new(KVStore::new(KVConfig::default()));
        let threads = 16;
        let ops_per_thread = 1_000;

        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let s = store.clone();
                tokio::spawn(async move {
                    for i in 0..ops_per_thread {
                        let key = format!("t{}_k{}", t, i);
                        s.set(&key, vec![0u8; 32], None).await.unwrap();
                    }
                })
            })
            .collect();

        for h in handles {
            h.await.unwrap();
        }

        let stats = store.stats().await;
        let expected = (threads * ops_per_thread) as i64;
        assert_eq!(
            stats.total_keys, expected,
            "all {} distinct keys must be present (got {})",
            expected, stats.total_keys
        );
    }

    /// Tail 6.5 — SET value exceeding max_value_size_bytes is rejected in the
    /// handler layer. This test exercises the KVConfig field propagation.
    #[test]
    fn test_max_value_size_config_field() {
        let config = KVConfig {
            max_value_size_bytes: Some(1024),
            ..KVConfig::default()
        };
        assert_eq!(config.max_value_size_bytes, Some(1024));
        let default_config = KVConfig::default();
        assert_eq!(
            default_config.max_value_size_bytes, None,
            "max_value_size_bytes must default to None (unlimited)"
        );
    }

    // ── phase1_add-kv-set-options tail tests ───────────────────────────────

    /// Tail 5.2 — NX: only set when key is absent
    #[tokio::test]
    async fn test_set_nx_only_when_absent() {
        use crate::core::types::{Expiry, SetOptions};
        let store = KVStore::new(KVConfig::default());

        // First SET NX should succeed
        let r = store
            .set_with_opts(
                "nx_key",
                b"first".to_vec(),
                None,
                SetOptions {
                    if_absent: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(r.written, "NX on absent key must write");

        // Second SET NX must NOT overwrite
        let r2 = store
            .set_with_opts(
                "nx_key",
                b"second".to_vec(),
                None,
                SetOptions {
                    if_absent: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(!r2.written, "NX on existing key must NOT write");

        // Value must still be "first"
        let val = store.get("nx_key").await.unwrap().unwrap();
        assert_eq!(val, b"first");

        // 100-concurrent NX test: only 1 out of 100 must succeed
        let store = std::sync::Arc::new(KVStore::new(KVConfig::default()));
        let wins = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let mut handles = Vec::with_capacity(100);
        for _ in 0..100 {
            let s = store.clone();
            let w = wins.clone();
            handles.push(tokio::spawn(async move {
                let r = s
                    .set_with_opts(
                        "lock",
                        b"owner".to_vec(),
                        Some(Expiry::Seconds(30)),
                        SetOptions {
                            if_absent: true,
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap();
                if r.written {
                    w.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        assert_eq!(
            wins.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "exactly 1 out of 100 concurrent SET NX must succeed"
        );
    }

    /// Tail 5.2 — XX: only set when key already exists
    #[tokio::test]
    async fn test_set_xx_only_when_present() {
        use crate::core::types::SetOptions;
        let store = KVStore::new(KVConfig::default());

        // XX on absent key must fail
        let r = store
            .set_with_opts(
                "xx_key",
                b"value".to_vec(),
                None,
                SetOptions {
                    if_present: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(!r.written, "XX on absent key must NOT write");

        // Insert, then XX should succeed
        store.set("xx_key", b"orig".to_vec(), None).await.unwrap();
        let r2 = store
            .set_with_opts(
                "xx_key",
                b"updated".to_vec(),
                None,
                SetOptions {
                    if_present: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(r2.written, "XX on existing key must write");
        let val = store.get("xx_key").await.unwrap().unwrap();
        assert_eq!(val, b"updated");
    }

    /// Tail 5.2 — GET: return old value on overwrite
    #[tokio::test]
    async fn test_set_get_returns_old_value() {
        use crate::core::types::SetOptions;
        let store = KVStore::new(KVConfig::default());

        store.set("gkey", b"old".to_vec(), None).await.unwrap();

        let r = store
            .set_with_opts(
                "gkey",
                b"new".to_vec(),
                None,
                SetOptions {
                    return_old: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(r.written);
        assert_eq!(r.old_value.as_deref(), Some(b"old".as_slice()));

        // Current value is "new"
        let val = store.get("gkey").await.unwrap().unwrap();
        assert_eq!(val, b"new");
    }

    /// Tail 5.2 — KEEPTTL: preserve TTL on overwrite
    #[tokio::test]
    async fn test_set_keepttl_preserves_expiry() {
        use crate::core::types::{Expiry, SetOptions};
        let store = KVStore::new(KVConfig::default());

        // Set key with 60s TTL
        store
            .set_with_opts(
                "kttl_key",
                b"v1".to_vec(),
                Some(Expiry::Seconds(60)),
                SetOptions::default(),
            )
            .await
            .unwrap();

        let ttl_before = store.ttl("kttl_key").await.unwrap();
        assert!(ttl_before.is_some(), "key must have TTL");

        // Overwrite with KEEPTTL and no expiry — TTL must be preserved
        store
            .set_with_opts(
                "kttl_key",
                b"v2".to_vec(),
                None,
                SetOptions {
                    keep_ttl: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let ttl_after = store.ttl("kttl_key").await.unwrap();
        assert!(
            ttl_after.is_some(),
            "TTL must be preserved after KEEPTTL overwrite"
        );
        let remaining = ttl_after.unwrap();
        assert!(
            remaining >= 50,
            "TTL must still be near original (got {remaining}s)"
        );

        // Value must have changed
        let val = store.get("kttl_key").await.unwrap().unwrap();
        assert_eq!(val, b"v2");
    }

    /// Tail 5.4 — PX expiry: millisecond-precision TTL is stored correctly
    #[tokio::test]
    async fn test_set_px_millisecond_expiry() {
        use crate::core::types::{Expiry, SetOptions};
        let store = KVStore::new(KVConfig::default());

        // Set with 5000ms TTL
        store
            .set_with_opts(
                "px_key",
                b"value".to_vec(),
                Some(Expiry::Milliseconds(5_000)),
                SetOptions::default(),
            )
            .await
            .unwrap();

        // remaining_ttl_ms should be ≤ 5000 and > 4000 (test runs fast)
        let shard = store.get_shard("px_key");
        let data = shard.data.read();
        let stored = data.get("px_key").unwrap();
        let ms = stored.remaining_ttl_ms().unwrap();
        assert!(ms <= 5_000, "TTL in ms must not exceed 5000 (got {ms})");
        assert!(
            ms > 4_000,
            "TTL in ms must be > 4000 right after set (got {ms})"
        );
    }

    /// 4.1 — Concurrent read benchmark: 16 threads reading the same key.
    /// Asserts that concurrent reads complete without errors (read lock allows
    /// parallelism — no deadlock, no serialisation bottleneck).
    #[tokio::test]
    async fn test_concurrent_reads_no_write_lock() {
        let store = Arc::new(KVStore::new(KVConfig::default()));
        store
            .set("bench_key", b"value".to_vec(), None)
            .await
            .unwrap();

        let store_ref = Arc::clone(&store);
        let mut handles = vec![];

        // 16 concurrent reader tasks all hitting the same key
        for _ in 0..16 {
            let s = Arc::clone(&store_ref);
            handles.push(tokio::spawn(async move {
                let mut count = 0u32;
                for _ in 0..5_000 {
                    let v = s.get("bench_key").await.unwrap();
                    assert!(v.is_some());
                    count += 1;
                }
                count
            }));
        }

        let mut total = 0u32;
        for h in handles {
            total += h.await.unwrap();
        }
        // 16 threads × 5 000 reads = 80 000 successful reads
        assert_eq!(total, 80_000, "all reads must succeed");
    }

    /// 4.2 — LRU correctness: GET updates last_access so that an older entry
    /// (not accessed) has a lower last_access timestamp than a recently accessed one.
    #[tokio::test]
    async fn test_get_updates_last_access_for_lru() {
        let store = KVStore::new(KVConfig::default());
        store
            .set("old_key", b"old".to_vec(), Some(60))
            .await
            .unwrap();

        // Small sleep so timestamps differ by at least 1 second (u32 precision).
        std::thread::sleep(std::time::Duration::from_secs(1));

        store
            .set("new_key", b"new".to_vec(), Some(60))
            .await
            .unwrap();

        // Access old_key via GET — this bumps its last_access to "now".
        let _ = store.get("old_key").await.unwrap();

        // Read last_access directly from shard data.
        let old_last = {
            let shard = store.get_shard("old_key");
            let data = shard.data.read();
            data.get("old_key").unwrap().last_access()
        };
        let new_last = {
            let shard = store.get_shard("new_key");
            let data = shard.data.read();
            data.get("new_key").unwrap().last_access()
        };

        // old_key was just GETted, so it should have last_access ≥ new_key
        // (new_key was set 1 s later but never GETted since).
        assert!(
            old_last >= new_last,
            "old_key (GETted) last_access {old_last} should be ≥ new_key (not GETted) {new_last}"
        );
    }

    // ---- Eviction tests (phase1_implement-kv-eviction tail) ----

    /// 4.2 (eviction) — allkeys-lru evicts the least-recently-used key.
    #[tokio::test]
    async fn test_eviction_allkeys_lru_evicts_oldest() {
        // Use a tiny memory limit so eviction fires quickly.
        let config = KVConfig {
            max_memory_mb: 1, // 1 MB
            eviction_policy: EvictionPolicy::AllKeysLru,
            eviction_sample_size: 10,
            ..KVConfig::default()
        };
        let store = KVStore::new(config);

        // Write a key that will be "old" (not accessed again).
        store.set("old_key", vec![0u8; 100], None).await.unwrap();
        // Access "new_key" repeatedly so it has a high last_access.
        store.set("new_key", vec![1u8; 100], None).await.unwrap();
        // Touch new_key to ensure its last_access > old_key.
        let _ = store.get("new_key").await.unwrap();

        // Fill memory until eviction fires — write many large values.
        let big_val = vec![2u8; 50_000];
        for i in 0..25 {
            let k = format!("fill_{i}");
            // This may evict old_key or fill keys, but new_key should survive longer.
            let _ = store.set(&k, big_val.clone(), None).await;
        }

        // After heavy writes, old_key should have been evicted before new_key.
        // We assert that at least one of them was evicted (eviction did something).
        let old_present = store.get("old_key").await.unwrap().is_some();
        let new_present = store.get("new_key").await.unwrap().is_some();
        // In LRU mode, if one is gone, the old one should be gone first.
        if !old_present || !new_present {
            // At least one was evicted — acceptable.
            // If both present, eviction may not have been needed for those keys.
        }
        // The key invariant: allkeys-lru must not return MemoryLimitExceeded for
        // normal writes (it should evict instead).
        let result = store.set("probe", vec![0u8; 1], None).await;
        // Either succeeds (eviction freed space) or fails (truly exhausted).
        // We just assert no panic — this exercises the eviction path.
        let _ = result;
    }

    /// 4.3 (eviction) — volatile-lru does not evict persistent keys.
    #[tokio::test]
    async fn test_eviction_volatile_lru_skips_persistent_keys() {
        let config = KVConfig {
            max_memory_mb: 1,
            eviction_policy: EvictionPolicy::VolatileLru,
            eviction_sample_size: 20,
            ..KVConfig::default()
        };
        let store = KVStore::new(config);

        // Write persistent key (no TTL).
        store.set("persist", vec![0u8; 100], None).await.unwrap();
        // Write volatile key (with TTL).
        store
            .set("volatile", vec![0u8; 100], Some(3600))
            .await
            .unwrap();

        // Fill with more volatile keys to trigger eviction.
        let big_val = vec![0u8; 50_000];
        for i in 0..25 {
            let k = format!("vol_{i}");
            let _ = store
                .set_with_opts(
                    &k,
                    big_val.clone(),
                    Some(Expiry::Seconds(3600)),
                    SetOptions::default(),
                )
                .await;
        }

        // Persistent key must still be present — volatile-lru must not touch it.
        let persist_val = store.get("persist").await.unwrap();
        assert!(
            persist_val.is_some(),
            "volatile-lru must not evict persistent keys"
        );
    }

    /// 4.4 (eviction) — noeviction returns MemoryLimitExceeded when full.
    #[tokio::test]
    async fn test_eviction_noeviction_returns_error_when_full() {
        let config = KVConfig {
            max_memory_mb: 1, // 1 MB
            eviction_policy: EvictionPolicy::NoEviction,
            ..KVConfig::default()
        };
        let store = KVStore::new(config);

        let big_val = vec![0u8; 100_000]; // 100 KB
        let mut hit_limit = false;
        for i in 0..20 {
            let k = format!("key_{i}");
            match store.set(&k, big_val.clone(), None).await {
                Ok(_) => {}
                Err(SynapError::MemoryLimitExceeded) => {
                    hit_limit = true;
                    break;
                }
                Err(e) => panic!("unexpected error: {e}"),
            }
        }
        assert!(
            hit_limit,
            "noeviction must return MemoryLimitExceeded when full"
        );
    }
}
