//! Sharded list store (64-way) over `ListValue`.
//!
//! Split out of the former monolithic `list.rs` (phase2 modularization).
//! `ListValue`/`ListRepr` and the packed-encoding thresholds live in the
//! parent module; this file holds only the store-level, sharded API.
use super::ListValue;
use crate::core::error::{Result, SynapError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::timeout;

const SHARD_COUNT: usize = 64;

/// Statistics for list operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListStats {
    pub total_lists: usize,
    pub total_elements: usize,
    pub lpush_count: u64,
    pub rpush_count: u64,
    pub lpop_count: u64,
    pub rpop_count: u64,
    pub lrange_count: u64,
    pub llen_count: u64,
    pub lindex_count: u64,
    pub lset_count: u64,
    pub ltrim_count: u64,
    pub lrem_count: u64,
    pub linsert_count: u64,
    pub rpoplpush_count: u64,
    pub blpop_count: u64,
    pub brpop_count: u64,
}

/// Lock-free per-op counters (phase12). Bumping a counter no longer takes a
/// global `RwLock` on the whole stats struct — every list mutation contended on
/// that one lock. The structural totals (`total_lists`/`total_elements`) are
/// recomputed on demand in [`ListStore::stats`], so only the counters live here.
#[derive(Default)]
struct AtomicListStats {
    lpush_count: AtomicU64,
    rpush_count: AtomicU64,
    lpop_count: AtomicU64,
    rpop_count: AtomicU64,
    lrange_count: AtomicU64,
    llen_count: AtomicU64,
    lindex_count: AtomicU64,
    lset_count: AtomicU64,
    ltrim_count: AtomicU64,
    lrem_count: AtomicU64,
    linsert_count: AtomicU64,
    rpoplpush_count: AtomicU64,
    blpop_count: AtomicU64,
    brpop_count: AtomicU64,
}

impl AtomicListStats {
    fn snapshot(&self) -> ListStats {
        ListStats {
            total_lists: 0,
            total_elements: 0,
            lpush_count: self.lpush_count.load(Ordering::Relaxed),
            rpush_count: self.rpush_count.load(Ordering::Relaxed),
            lpop_count: self.lpop_count.load(Ordering::Relaxed),
            rpop_count: self.rpop_count.load(Ordering::Relaxed),
            lrange_count: self.lrange_count.load(Ordering::Relaxed),
            llen_count: self.llen_count.load(Ordering::Relaxed),
            lindex_count: self.lindex_count.load(Ordering::Relaxed),
            lset_count: self.lset_count.load(Ordering::Relaxed),
            ltrim_count: self.ltrim_count.load(Ordering::Relaxed),
            lrem_count: self.lrem_count.load(Ordering::Relaxed),
            linsert_count: self.linsert_count.load(Ordering::Relaxed),
            rpoplpush_count: self.rpoplpush_count.load(Ordering::Relaxed),
            blpop_count: self.blpop_count.load(Ordering::Relaxed),
            brpop_count: self.brpop_count.load(Ordering::Relaxed),
        }
    }
}

/// Sharded list store with 64-way concurrency
pub struct ListStore {
    shards: Vec<Arc<RwLock<HashMap<String, ListValue>>>>,
    stats: Arc<AtomicListStats>,
    /// Broadcast channel for notifying blocked waiters
    /// Key: list key
    notify_tx: Arc<RwLock<HashMap<String, broadcast::Sender<()>>>>,
    /// Number of keys with a notify channel — lets `notify_waiters` skip the
    /// map entirely when no blocking pop has ever registered (the common case).
    notify_channels: AtomicU64,
    /// Shared cross-datatype memory budget (audit M-018).
    mem: Option<crate::core::GlobalMemory>,
    mem_bytes: Arc<std::sync::atomic::AtomicI64>,
    /// Optional keyspace-notification publisher (Redis `notify-keyspace-events`).
    keyspace_notifier: Option<Arc<crate::core::KeyspaceNotifier>>,
}

impl Default for ListStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ListStore {
    /// Dump every list (key -> ListValue) across all shards, for snapshotting.
    pub fn dump(&self) -> HashMap<String, ListValue> {
        let mut out = HashMap::new();
        for shard in self.shards.iter() {
            let guard = shard.read();
            for (key, v) in guard.iter() {
                out.insert(key.clone(), v.clone());
            }
        }
        out
    }

    /// Create new list store
    pub fn new() -> Self {
        let mut shards = Vec::with_capacity(SHARD_COUNT);
        for _ in 0..SHARD_COUNT {
            shards.push(Arc::new(RwLock::new(HashMap::new())));
        }

        Self {
            shards,
            stats: Arc::new(AtomicListStats::default()),
            notify_tx: Arc::new(RwLock::new(HashMap::new())),
            notify_channels: AtomicU64::new(0),
            mem: None,
            mem_bytes: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            keyspace_notifier: None,
        }
    }

    /// Attach the shared cross-datatype memory budget (audit M-018).
    pub fn with_global_memory(mut self, mem: crate::core::GlobalMemory) -> Self {
        mem.register(Arc::clone(&self.mem_bytes));
        self.mem = Some(mem);
        self
    }

    /// Attach a keyspace-notification publisher so list mutations publish
    /// `l`-class events. A no-op when `notifier` is `None`.
    pub fn with_keyspace_notifier(
        mut self,
        notifier: Option<Arc<crate::core::KeyspaceNotifier>>,
    ) -> Self {
        self.keyspace_notifier = notifier;
        self
    }

    /// Publish a list keyspace notification for `key` if a notifier is attached.
    #[inline]
    fn notify_keyspace(&self, event: &str, key: &str) {
        if let Some(ref n) = self.keyspace_notifier {
            n.notify(crate::core::EventClass::List, event, key);
        }
    }

    /// Total payload bytes currently held (keys + element values across shards).
    pub fn memory_bytes(&self) -> usize {
        let mut total = 0usize;
        for shard in self.shards.iter() {
            for (key, v) in shard.read().iter() {
                total += key.len();
                total += v.element_bytes();
            }
        }
        total
    }

    /// Recompute this store's accounted memory into its registered counter.
    pub fn refresh_memory(&self) {
        if self.mem.is_some() {
            self.mem_bytes.store(
                self.memory_bytes() as i64,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
    }

    /// Refuse a growing write when the shared budget is already over the cap.
    fn check_admit(&self, incoming: usize) -> Result<()> {
        if let Some(m) = &self.mem
            && m.would_exceed(incoming as i64)
        {
            return Err(SynapError::MemoryLimitExceeded);
        }
        Ok(())
    }

    /// Get shard index for key
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % SHARD_COUNT
    }

    /// Get shard for key
    fn shard(&self, key: &str) -> &Arc<RwLock<HashMap<String, ListValue>>> {
        &self.shards[self.shard_index(key)]
    }

    /// Notify blocked waiters on a key.
    ///
    /// Fast path: when no blocking pop has ever registered a channel
    /// (`notify_channels == 0` — the common case for non-blocking workloads),
    /// skip the lock + hash lookup entirely, so LPUSH/RPUSH never touch the map.
    fn notify_waiters(&self, key: &str) {
        if self.notify_channels.load(Ordering::Relaxed) == 0 {
            return;
        }
        if let Some(tx) = self.notify_tx.read().get(key) {
            let _ = tx.send(());
        }
    }

    /// Create or get broadcast channel for a key
    fn get_or_create_channel(&self, key: &str) -> broadcast::Receiver<()> {
        let mut notify = self.notify_tx.write();
        let tx = notify.entry(key.to_string()).or_insert_with(|| {
            self.notify_channels.fetch_add(1, Ordering::Relaxed);
            broadcast::channel(100).0
        });
        tx.subscribe()
    }

    /// LPUSH - Push element(s) to left (front)
    pub fn lpush(&self, key: &str, values: Vec<Vec<u8>>, only_if_exists: bool) -> Result<usize> {
        self.check_admit(values.iter().map(|v| v.len()).sum())?;
        let shard = self.shard(key);
        let mut map = shard.write();

        // Check if key exists when using LPUSHX
        if only_if_exists && !map.contains_key(key) {
            return Ok(0);
        }

        let list = map
            .entry(key.to_string())
            .or_insert_with(|| ListValue::new(None));

        // Check expiration
        if list.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        // Push all values
        for value in values {
            list.lpush(value);
        }

        let len = list.len();
        self.stats.lpush_count.fetch_add(1, Ordering::Relaxed);

        // Notify blocked waiters
        drop(map);
        self.notify_waiters(key);
        self.notify_keyspace("lpush", key);

        Ok(len)
    }

    /// RPUSH - Push element(s) to right (back)
    pub fn rpush(&self, key: &str, values: Vec<Vec<u8>>, only_if_exists: bool) -> Result<usize> {
        self.check_admit(values.iter().map(|v| v.len()).sum())?;
        let shard = self.shard(key);
        let mut map = shard.write();

        // Check if key exists when using RPUSHX
        if only_if_exists && !map.contains_key(key) {
            return Ok(0);
        }

        let list = map
            .entry(key.to_string())
            .or_insert_with(|| ListValue::new(None));

        // Check expiration
        if list.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        // Push all values
        for value in values {
            list.rpush(value);
        }

        let len = list.len();
        self.stats.rpush_count.fetch_add(1, Ordering::Relaxed);

        // Notify blocked waiters
        drop(map);
        self.notify_waiters(key);
        self.notify_keyspace("rpush", key);

        Ok(len)
    }

    /// LPOP - Pop element from left (front)
    pub fn lpop(&self, key: &str, count: Option<usize>) -> Result<Vec<Vec<u8>>> {
        let shard = self.shard(key);
        let mut map = shard.write();

        let list = map.get_mut(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        let count = count.unwrap_or(1);
        let mut result = Vec::new();

        for _ in 0..count {
            if let Some(value) = list.lpop() {
                result.push(value);
            } else {
                break;
            }
        }

        // Remove empty lists
        if list.is_empty() {
            map.remove(key);
        }

        self.stats.lpop_count.fetch_add(1, Ordering::Relaxed);

        drop(map);
        if result.is_empty() {
            Err(SynapError::NotFound)
        } else {
            self.notify_keyspace("lpop", key);
            Ok(result)
        }
    }

    /// RPOP - Pop element from right (back)
    pub fn rpop(&self, key: &str, count: Option<usize>) -> Result<Vec<Vec<u8>>> {
        let shard = self.shard(key);
        let mut map = shard.write();

        let list = map.get_mut(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        let count = count.unwrap_or(1);
        let mut result = Vec::new();

        for _ in 0..count {
            if let Some(value) = list.rpop() {
                result.push(value);
            } else {
                break;
            }
        }

        // Remove empty lists
        if list.is_empty() {
            map.remove(key);
        }

        self.stats.rpop_count.fetch_add(1, Ordering::Relaxed);

        drop(map);
        if result.is_empty() {
            Err(SynapError::NotFound)
        } else {
            self.notify_keyspace("rpop", key);
            Ok(result)
        }
    }

    /// LRANGE - Get range of elements
    pub fn lrange(&self, key: &str, start: i64, stop: i64) -> Result<Vec<Vec<u8>>> {
        let shard = self.shard(key);
        let map = shard.read();

        let list = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        self.stats.lrange_count.fetch_add(1, Ordering::Relaxed);
        Ok(list.lrange(start, stop))
    }

    /// LLEN - Get list length
    pub fn llen(&self, key: &str) -> Result<usize> {
        let shard = self.shard(key);
        let map = shard.read();

        let list = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        self.stats.llen_count.fetch_add(1, Ordering::Relaxed);
        Ok(list.len())
    }

    /// LINDEX - Get element at index
    pub fn lindex(&self, key: &str, index: i64) -> Result<Vec<u8>> {
        let shard = self.shard(key);
        let map = shard.read();

        let list = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        self.stats.lindex_count.fetch_add(1, Ordering::Relaxed);
        list.lindex(index).ok_or(SynapError::IndexOutOfRange)
    }

    /// LSET - Set element at index
    pub fn lset(&self, key: &str, index: i64, value: Vec<u8>) -> Result<()> {
        let shard = self.shard(key);
        let mut map = shard.write();

        let list = map.get_mut(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        self.stats.lset_count.fetch_add(1, Ordering::Relaxed);
        list.lset(index, value)
    }

    /// LTRIM - Trim list to keep only range [start, stop]
    pub fn ltrim(&self, key: &str, start: i64, stop: i64) -> Result<()> {
        let shard = self.shard(key);
        let mut map = shard.write();

        let list = map.get_mut(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        self.stats.ltrim_count.fetch_add(1, Ordering::Relaxed);
        list.ltrim(start, stop);

        // Remove empty lists
        if list.is_empty() {
            map.remove(key);
        }

        Ok(())
    }

    /// LREM - Remove count occurrences of value
    pub fn lrem(&self, key: &str, count: i64, value: Vec<u8>) -> Result<usize> {
        let shard = self.shard(key);
        let mut map = shard.write();

        let list = map.get_mut(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        self.stats.lrem_count.fetch_add(1, Ordering::Relaxed);
        let removed = list.lrem(count, &value);

        // Remove empty lists
        if list.is_empty() {
            map.remove(key);
        }

        Ok(removed)
    }

    /// LINSERT - Insert value before/after pivot
    pub fn linsert(
        &self,
        key: &str,
        before: bool,
        pivot: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<usize> {
        let shard = self.shard(key);
        let mut map = shard.write();

        let list = map.get_mut(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        self.stats.linsert_count.fetch_add(1, Ordering::Relaxed);
        list.linsert(before, &pivot, value)
    }

    /// LPOS - Find position of value
    pub fn lpos(&self, key: &str, value: Vec<u8>) -> Result<Option<usize>> {
        let shard = self.shard(key);
        let map = shard.read();

        let list = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if list.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        Ok(list.lpos(&value))
    }

    /// RPOPLPUSH - Atomically pop from source and push to destination
    pub fn rpoplpush(&self, source: &str, destination: &str) -> Result<Vec<u8>> {
        // Lock both shards in consistent order to prevent deadlocks
        let source_idx = self.shard_index(source);
        let dest_idx = self.shard_index(destination);

        // Handle same shard case
        if source_idx == dest_idx {
            let shard = &self.shards[source_idx];
            let mut map = shard.write();

            // Pop from source
            let source_list = map.get_mut(source).ok_or(SynapError::NotFound)?;

            if source_list.is_expired() {
                map.remove(source);
                return Err(SynapError::KeyExpired);
            }

            let value = source_list.rpop().ok_or(SynapError::NotFound)?;

            // Remove empty source list
            let should_remove_source = source_list.is_empty();
            if should_remove_source {
                map.remove(source);
            }

            // Push to destination (handle expiration first)
            if let Some(existing_list) = map.get(destination)
                && existing_list.is_expired()
            {
                map.remove(destination);
            }

            let dest_list = map
                .entry(destination.to_string())
                .or_insert_with(|| ListValue::new(None));

            dest_list.lpush(value.clone());

            self.stats.rpoplpush_count.fetch_add(1, Ordering::Relaxed);

            drop(map);
            self.notify_waiters(destination);

            return Ok(value);
        }

        // Different shards - lock in order
        let (first_idx, second_idx) = if source_idx < dest_idx {
            (source_idx, dest_idx)
        } else {
            (dest_idx, source_idx)
        };

        let first_shard = &self.shards[first_idx];
        let second_shard = &self.shards[second_idx];

        let mut first_map = first_shard.write();
        let mut second_map = second_shard.write();

        // Get correct map references based on actual source/dest indices
        let (source_map, dest_map) = if source_idx < dest_idx {
            (&mut *first_map, &mut *second_map)
        } else {
            (&mut *second_map, &mut *first_map)
        };

        // Pop from source
        let source_list = source_map.get_mut(source).ok_or(SynapError::NotFound)?;

        if source_list.is_expired() {
            source_map.remove(source);
            return Err(SynapError::KeyExpired);
        }

        let value = source_list.rpop().ok_or(SynapError::NotFound)?;

        // Remove empty source list
        if source_list.is_empty() {
            source_map.remove(source);
        }

        // Push to destination (handle expiration first)
        if let Some(existing_list) = dest_map.get(destination)
            && existing_list.is_expired()
        {
            dest_map.remove(destination);
        }

        let dest_list = dest_map
            .entry(destination.to_string())
            .or_insert_with(|| ListValue::new(None));

        dest_list.lpush(value.clone());

        self.stats.rpoplpush_count.fetch_add(1, Ordering::Relaxed);

        drop(first_map);
        drop(second_map);
        self.notify_waiters(destination);

        Ok(value)
    }

    /// BLPOP - Blocking pop from left with timeout
    pub async fn blpop(
        &self,
        keys: Vec<String>,
        timeout_secs: Option<u64>,
    ) -> Result<(String, Vec<u8>)> {
        // Try immediate pop first
        for key in &keys {
            if let Ok(mut values) = self.lpop(key, Some(1))
                && !values.is_empty()
            {
                self.stats.blpop_count.fetch_add(1, Ordering::Relaxed);
                return Ok((key.clone(), values.remove(0)));
            }
        }

        // No immediate result, wait for notification
        if let Some(timeout_secs) = timeout_secs {
            // Create receivers for all keys
            let mut receivers: Vec<_> =
                keys.iter().map(|k| self.get_or_create_channel(k)).collect();

            let duration = Duration::from_secs(timeout_secs);
            let result = timeout(duration, async {
                loop {
                    // Wait for any notification
                    for rx in &mut receivers {
                        let _ = rx.recv().await;
                    }

                    // Try to pop from any key
                    for key in &keys {
                        if let Ok(mut values) = self.lpop(key, Some(1))
                            && !values.is_empty()
                        {
                            return Ok((key.clone(), values.remove(0)));
                        }
                    }
                }
            })
            .await;

            match result {
                Ok(Ok(value)) => {
                    self.stats.blpop_count.fetch_add(1, Ordering::Relaxed);
                    Ok(value)
                }
                Ok(Err(e)) => Err(e),
                Err(_) => Err(SynapError::Timeout),
            }
        } else {
            // Wait indefinitely
            let mut receivers: Vec<_> =
                keys.iter().map(|k| self.get_or_create_channel(k)).collect();

            loop {
                // Wait for any notification
                for rx in &mut receivers {
                    let _ = rx.recv().await;
                }

                // Try to pop from any key
                for key in &keys {
                    if let Ok(mut values) = self.lpop(key, Some(1))
                        && !values.is_empty()
                    {
                        self.stats.blpop_count.fetch_add(1, Ordering::Relaxed);
                        return Ok((key.clone(), values.remove(0)));
                    }
                }
            }
        }
    }

    /// BRPOPLPUSH - Blocking RPOPLPUSH with timeout
    pub async fn brpoplpush(
        &self,
        source: &str,
        destination: &str,
        timeout_secs: Option<u64>,
    ) -> Result<Vec<u8>> {
        // Try immediate operation first
        if let Ok(value) = self.rpoplpush(source, destination) {
            return Ok(value);
        }

        // No immediate result, wait for notification
        let rx = self.get_or_create_channel(source);

        if let Some(timeout_secs) = timeout_secs {
            let duration = Duration::from_secs(timeout_secs);
            let result = timeout(duration, async {
                let mut receiver = rx;
                loop {
                    let _ = receiver.recv().await;

                    // Try operation after notification
                    if let Ok(value) = self.rpoplpush(source, destination) {
                        return Ok(value);
                    }
                }
            })
            .await;

            match result {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(SynapError::Timeout),
            }
        } else {
            // Wait indefinitely
            let mut receiver = rx;
            loop {
                let _ = receiver.recv().await;

                if let Ok(value) = self.rpoplpush(source, destination) {
                    return Ok(value);
                }
            }
        }
    }

    /// BRPOP - Blocking pop from right with timeout
    pub async fn brpop(
        &self,
        keys: Vec<String>,
        timeout_secs: Option<u64>,
    ) -> Result<(String, Vec<u8>)> {
        // Try immediate pop first
        for key in &keys {
            if let Ok(mut values) = self.rpop(key, Some(1))
                && !values.is_empty()
            {
                self.stats.brpop_count.fetch_add(1, Ordering::Relaxed);
                return Ok((key.clone(), values.remove(0)));
            }
        }

        // No immediate result, wait for notification
        if let Some(timeout_secs) = timeout_secs {
            let mut receivers: Vec<_> =
                keys.iter().map(|k| self.get_or_create_channel(k)).collect();

            let duration = Duration::from_secs(timeout_secs);
            let result = timeout(duration, async {
                loop {
                    for rx in &mut receivers {
                        let _ = rx.recv().await;
                    }

                    for key in &keys {
                        if let Ok(mut values) = self.rpop(key, Some(1))
                            && !values.is_empty()
                        {
                            return Ok((key.clone(), values.remove(0)));
                        }
                    }
                }
            })
            .await;

            match result {
                Ok(Ok(value)) => {
                    self.stats.brpop_count.fetch_add(1, Ordering::Relaxed);
                    Ok(value)
                }
                Ok(Err(e)) => Err(e),
                Err(_) => Err(SynapError::Timeout),
            }
        } else {
            let mut receivers: Vec<_> =
                keys.iter().map(|k| self.get_or_create_channel(k)).collect();

            loop {
                for rx in &mut receivers {
                    let _ = rx.recv().await;
                }

                for key in &keys {
                    if let Ok(mut values) = self.rpop(key, Some(1))
                        && !values.is_empty()
                    {
                        self.stats.brpop_count.fetch_add(1, Ordering::Relaxed);
                        return Ok((key.clone(), values.remove(0)));
                    }
                }
            }
        }
    }

    /// Get statistics
    pub fn stats(&self) -> ListStats {
        let mut stats = self.stats.snapshot();

        // Count total lists and elements
        let mut total_lists = 0;
        let mut total_elements = 0;

        for shard in &self.shards {
            let map = shard.read();
            total_lists += map.len();
            for list in map.values() {
                if !list.is_expired() {
                    total_elements += list.len();
                }
            }
        }

        stats.total_lists = total_lists;
        stats.total_elements = total_elements;

        stats
    }

    /// Delete a list
    pub fn delete(&self, key: &str) -> Result<bool> {
        let shard = self.shard(key);
        let mut map = shard.write();
        Ok(map.remove(key).is_some())
    }

    /// Check if list exists
    pub fn exists(&self, key: &str) -> bool {
        let shard = self.shard(key);
        let map = shard.read();
        if let Some(list) = map.get(key) {
            !list.is_expired()
        } else {
            false
        }
    }
}
