use super::super::error::{Result, SynapError};
use super::super::types::{
    AtomicKVStats, EvictionPolicy, Expiry, KVConfig, KVStats, KeyBuf, SetOptions, SetResult,
    StoredValue,
};
use super::storage::{KVShard, SHARD_COUNT, ShardStorage};
use ahash::RandomState;
use std::cmp::Reverse;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Process-wide ahash seed for consistent shard selection across calls.
fn shard_hasher() -> &'static RandomState {
    static HASHER: OnceLock<RandomState> = OnceLock::new();
    HASHER.get_or_init(RandomState::new)
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
        let mut hasher = shard_hasher().build_hasher();
        hasher.write(key.as_bytes());
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

        shard.track_ttl(&stored, &key);
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
        shard.track_ttl(&stored, key);
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
        let stored = StoredValue::new(new_data, existing_ttl_secs);
        shard.track_ttl(&stored, key);
        data.insert(key.to_string(), stored);

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

    /// Get multiple values.
    ///
    /// Shard-aware: keys are bucketed by shard index, and each shard's
    /// `RwLock` is acquired exactly once for the entire batch instead of
    /// once per key. This collapses what used to be `O(n)` lock-acquire
    /// cycles into `O(min(n, SHARD_COUNT))` and lets the read lock cover
    /// many lookups, dramatically reducing contention with concurrent
    /// writers on the same shard.
    pub async fn mget(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        debug!("MGET count={}", keys.len());

        let mut results: Vec<Option<Vec<u8>>> = vec![None; keys.len()];

        // 1. Cluster routing check for every key — fail fast on the
        //    first wrong-owner key, matching single-key get() semantics.
        for key in keys {
            self.check_cluster_routing(key)?;
        }

        // 2. L1 cache pass — anything served from cache skips the shard.
        let mut pending: Vec<(usize, &str)> = Vec::with_capacity(keys.len());
        if let Some(ref cache) = self.cache {
            for (i, key) in keys.iter().enumerate() {
                if let Some(cached) = cache.get(key) {
                    self.stats.gets.fetch_add(1, Ordering::Relaxed);
                    self.stats.hits.fetch_add(1, Ordering::Relaxed);
                    results[i] = Some(cached);
                } else {
                    pending.push((i, key.as_str()));
                }
            }
        } else {
            for (i, key) in keys.iter().enumerate() {
                pending.push((i, key.as_str()));
            }
        }

        if pending.is_empty() {
            return Ok(results);
        }

        // 3. Bucket pending keys by shard.
        let mut buckets: Vec<Vec<(usize, &str)>> = (0..SHARD_COUNT).map(|_| Vec::new()).collect();
        for (orig, key) in pending {
            let idx = self.shard_for_key(key);
            buckets[idx].push((orig, key));
        }

        // 4. Per-shard pass — single read lock per non-empty shard.
        for (shard_idx, bucket) in buckets.iter().enumerate() {
            if bucket.is_empty() {
                continue;
            }
            let shard = &self.shards[shard_idx];

            // Keys that need cold-path eviction (expired). Stored with
            // their original input index so race recovery can populate
            // the result slot.
            let mut expired: Vec<(usize, &str)> = Vec::new();

            // Hot path: read lock spans the entire bucket.
            {
                let data = shard.data.read();
                for &(orig, key) in bucket {
                    self.stats.gets.fetch_add(1, Ordering::Relaxed);
                    match data.get(key) {
                        Some(value) if !value.is_expired() => {
                            // Atomic LRU update — safe under read lock.
                            value.update_access();
                            self.stats.hits.fetch_add(1, Ordering::Relaxed);
                            let value_data = value.data().to_vec();
                            let ttl = value.ttl_remaining();
                            if let Some(ref cache) = self.cache {
                                cache.put(key.to_string(), value_data.clone(), ttl);
                            }
                            results[orig] = Some(value_data);
                        }
                        Some(_) => {
                            // Present but expired — drop the read lock,
                            // then evict under a write lock below.
                            expired.push((orig, key));
                        }
                        None => {
                            self.stats.misses.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }

            // Cold path: only when at least one key needs eviction.
            if !expired.is_empty() {
                let mut data = shard.data.write();
                for (orig, key) in expired {
                    // Re-check under the write lock — another writer may
                    // have removed or refreshed the entry between locks.
                    let still_expired = match data.get(key) {
                        Some(v) if v.is_expired() => true,
                        Some(v) => {
                            // Race: another writer refreshed the key.
                            v.update_access();
                            self.stats.hits.fetch_add(1, Ordering::Relaxed);
                            let value_data = v.data().to_vec();
                            let ttl = v.ttl_remaining();
                            if let Some(ref cache) = self.cache {
                                cache.put(key.to_string(), value_data.clone(), ttl);
                            }
                            results[orig] = Some(value_data);
                            false
                        }
                        None => {
                            // Already removed by someone else.
                            self.stats.misses.fetch_add(1, Ordering::Relaxed);
                            false
                        }
                    };
                    if still_expired {
                        if let Some(removed) = data.remove(key) {
                            let removed_size = self.estimate_entry_size(key, &removed);
                            self.stats.total_keys.fetch_sub(1, Ordering::Relaxed);
                            self.stats
                                .total_memory_bytes
                                .fetch_sub(removed_size as i64, Ordering::Relaxed);
                        }
                        self.stats.misses.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
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

    /// Clean up expired keys.
    ///
    /// For `Small` (HashMap) shards the per-shard TTL min-heap is the
    /// primary expiration driver: we pop entries whose `expires_at` has
    /// passed, verify they are still live and still expired, and remove
    /// them. Stale heap entries (from key overwrites or deletes) are
    /// silently discarded — no fix-up on the write path.
    ///
    /// For `Large` (RadixTrie) shards the heap may be empty (keys
    /// inserted before the upgrade did not push to the heap), so we
    /// fall back to the original probabilistic sampling path.
    async fn cleanup_expired(&self) {
        const SAMPLE_SIZE: usize = 20;
        const MAX_ITERATIONS: usize = 16;
        const MAX_HEAP_POPS: usize = 256;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut total_expired = 0;

        for shard in self.shards.iter() {
            // --- Heap-driven eviction (fast path) ---
            let mut heap_evicted = 0;
            {
                let mut heap = shard.ttl_heap.lock();
                let mut keys_to_remove: Vec<KeyBuf> = Vec::new();

                while heap_evicted < MAX_HEAP_POPS {
                    match heap.peek() {
                        Some(&Reverse((exp, _))) if exp <= now_ms => {}
                        _ => break,
                    }
                    let Reverse((exp, key)) = heap.pop().expect("just peeked");

                    // Validate against live data: the key may have been
                    // deleted, overwritten with a new TTL, or converted
                    // to Persistent. Only evict if the stored expires_at
                    // matches the heap entry.
                    let data = shard.data.read();
                    match data.get(key.as_str()) {
                        Some(v) if v.expires_at_ms() == Some(exp) && v.is_expired() => {
                            keys_to_remove.push(key);
                        }
                        _ => {
                            // Stale heap entry — discard.
                        }
                    }
                    heap_evicted += 1;
                }

                // Batch-remove under a single write lock.
                if !keys_to_remove.is_empty() {
                    let mut data = shard.data.write();
                    for key in &keys_to_remove {
                        if let Some(removed_val) = data.remove(key.as_str()) {
                            let removed_size = self.estimate_entry_size(key.as_str(), &removed_val);
                            self.stats
                                .total_memory_bytes
                                .fetch_sub(removed_size as i64, Ordering::Relaxed);
                            total_expired += 1;
                        }
                    }
                }
            }

            // --- Sampling fallback (for Large/trie shards or heap lag) ---
            {
                let is_large = matches!(*shard.data.read(), ShardStorage::Large(_));
                if is_large {
                    for _ in 0..MAX_ITERATIONS {
                        let mut expired_keys = Vec::new();

                        {
                            let data = shard.data.read();
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

                        if expired_keys.len() < SAMPLE_SIZE / 4 {
                            break;
                        }
                    }
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

        // Clear all shards (data + TTL heap)
        for shard in self.shards.iter() {
            let mut data = shard.data.write();
            data.clear();
            shard.ttl_heap.lock().clear();
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
            shard.track_ttl(&new_value, key);
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
#[path = "store_tests.rs"]
mod store_tests;
