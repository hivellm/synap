//! List data structure implementation for Synap
//!
//! Provides Redis-compatible list operations (LPUSH, RPUSH, LPOP, RPOP, LRANGE, etc.)
//! Storage: VecDeque for O(1) push/pop at both ends
//!
//! # Performance Targets
//! - LPUSH/RPUSH: <100µs p99 latency
//! - LPOP/RPOP: <100µs p99 latency
//! - LRANGE (100 items): <500µs p99 latency
//! - BLPOP (no wait): <100µs p99 latency
//!
//! # Architecture
//! ```text
//! ListStore
//!   ├─ 64 shards (Arc<RwLock<HashMap<key, ListValue>>>)
//!   └─ TTL applies to entire list
//! ```

use super::error::{Result, SynapError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::timeout;

const SHARD_COUNT: usize = 64;

/// List value stored in a single key
/// Contains ordered elements with push/pop at both ends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListValue {
    /// Ordered elements (front = left, back = right)
    pub elements: VecDeque<Vec<u8>>,
    /// TTL for entire list
    pub ttl_secs: Option<u64>,
    /// Creation timestamp
    pub created_at: u64,
    /// Last modification timestamp
    pub updated_at: u64,
}

impl ListValue {
    /// Create new list value
    pub fn new(ttl_secs: Option<u64>) -> Self {
        let now = Self::current_timestamp();
        Self {
            elements: VecDeque::new(),
            ttl_secs,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get current Unix timestamp
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Check if list has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_secs {
            let now = Self::current_timestamp();
            now >= self.created_at + ttl
        } else {
            false
        }
    }

    /// Get number of elements
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if list is empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Push element to left (front)
    pub fn lpush(&mut self, value: Vec<u8>) {
        self.updated_at = Self::current_timestamp();
        self.elements.push_front(value);
    }

    /// Push element to right (back)
    pub fn rpush(&mut self, value: Vec<u8>) {
        self.updated_at = Self::current_timestamp();
        self.elements.push_back(value);
    }

    /// Pop element from left (front)
    pub fn lpop(&mut self) -> Option<Vec<u8>> {
        self.updated_at = Self::current_timestamp();
        self.elements.pop_front()
    }

    /// Pop element from right (back)
    pub fn rpop(&mut self) -> Option<Vec<u8>> {
        self.updated_at = Self::current_timestamp();
        self.elements.pop_back()
    }

    /// Get range of elements (0-indexed, inclusive)
    /// Supports negative indices (-1 = last element)
    pub fn lrange(&self, start: i64, stop: i64) -> Vec<Vec<u8>> {
        let len = self.elements.len() as i64;
        if len == 0 {
            return Vec::new();
        }

        // Normalize negative indices
        let start = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len - 1)
        };

        let stop = if stop < 0 {
            (len + stop).max(-1)
        } else {
            stop.min(len - 1)
        };

        if start > stop || start >= len {
            return Vec::new();
        }

        self.elements
            .iter()
            .skip(start as usize)
            .take((stop - start + 1) as usize)
            .cloned()
            .collect()
    }

    /// Get element at index (0-indexed)
    /// Supports negative indices (-1 = last element)
    pub fn lindex(&self, index: i64) -> Option<&Vec<u8>> {
        let len = self.elements.len() as i64;
        let idx = if index < 0 {
            (len + index).max(0)
        } else {
            index
        };

        if idx >= len || idx < 0 {
            None
        } else {
            self.elements.get(idx as usize)
        }
    }

    /// Set element at index
    pub fn lset(&mut self, index: i64, value: Vec<u8>) -> Result<()> {
        let len = self.elements.len() as i64;
        let idx = if index < 0 {
            (len + index).max(0)
        } else {
            index
        };

        if idx >= len || idx < 0 {
            return Err(SynapError::IndexOutOfRange);
        }

        self.updated_at = Self::current_timestamp();
        self.elements[idx as usize] = value;
        Ok(())
    }

    /// Trim list to keep only elements in range [start, stop]
    pub fn ltrim(&mut self, start: i64, stop: i64) {
        let len = self.elements.len() as i64;
        if len == 0 {
            return;
        }

        // Normalize indices
        let start = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len - 1)
        };

        let stop = if stop < 0 {
            (len + stop).max(-1)
        } else {
            stop.min(len - 1)
        };

        if start > stop || start >= len {
            self.elements.clear();
            return;
        }

        self.updated_at = Self::current_timestamp();

        // Remove elements after stop+1
        let keep_len = (stop + 1) as usize;
        self.elements.truncate(keep_len);

        // Remove elements before start
        for _ in 0..start {
            self.elements.pop_front();
        }
    }

    /// Remove count occurrences of value
    /// count > 0: remove from head to tail
    /// count < 0: remove from tail to head
    /// count = 0: remove all occurrences
    pub fn lrem(&mut self, count: i64, value: &[u8]) -> usize {
        self.updated_at = Self::current_timestamp();
        let mut removed = 0;

        if count == 0 {
            // Remove all occurrences
            self.elements.retain(|elem| elem != value);
            removed = self.elements.len();
        } else if count > 0 {
            // Remove from head to tail
            let mut i = 0;
            while i < self.elements.len() && removed < count as usize {
                if self.elements[i] == value {
                    self.elements.remove(i);
                    removed += 1;
                } else {
                    i += 1;
                }
            }
        } else {
            // Remove from tail to head
            let mut i = self.elements.len();
            let target = count.unsigned_abs() as usize;
            while i > 0 && removed < target {
                i -= 1;
                if self.elements[i] == value {
                    self.elements.remove(i);
                    removed += 1;
                }
            }
        }

        removed
    }

    /// Insert value before or after pivot
    pub fn linsert(&mut self, before: bool, pivot: &[u8], value: Vec<u8>) -> Result<usize> {
        if let Some(pos) = self.elements.iter().position(|elem| elem == pivot) {
            self.updated_at = Self::current_timestamp();
            let insert_pos = if before { pos } else { pos + 1 };
            self.elements.insert(insert_pos, value);
            Ok(self.elements.len())
        } else {
            Err(SynapError::NotFound)
        }
    }

    /// Find position of value (first occurrence, 0-indexed)
    pub fn lpos(&self, value: &[u8]) -> Option<usize> {
        self.elements.iter().position(|elem| elem == value)
    }
}

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
                for e in v.elements.iter() {
                    total += e.len();
                }
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
        list.lindex(index)
            .cloned()
            .ok_or(SynapError::IndexOutOfRange)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lpush_rpush() {
        let store = ListStore::new();

        // Test LPUSH
        let result = store.lpush("mylist", vec![b"world".to_vec()], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        let result = store.lpush("mylist", vec![b"hello".to_vec()], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);

        // Test RPUSH
        let result = store.rpush("mylist", vec![b"!".to_vec()], false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);

        // Verify order: ["hello", "world", "!"]
        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(
            range,
            vec![b"hello".to_vec(), b"world".to_vec(), b"!".to_vec()]
        );
    }

    #[test]
    fn test_lpop_rpop() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()],
                false,
            )
            .unwrap();

        // Test LPOP
        let result = store.lpop("mylist", Some(1)).unwrap();
        assert_eq!(result, vec![b"one".to_vec()]);

        // Test RPOP
        let result = store.rpop("mylist", Some(1)).unwrap();
        assert_eq!(result, vec![b"three".to_vec()]);

        // Only "two" should remain
        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range, vec![b"two".to_vec()]);
    }

    #[test]
    fn test_lrange() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![
                    b"0".to_vec(),
                    b"1".to_vec(),
                    b"2".to_vec(),
                    b"3".to_vec(),
                    b"4".to_vec(),
                ],
                false,
            )
            .unwrap();

        // Test positive indices
        let range = store.lrange("mylist", 1, 3).unwrap();
        assert_eq!(range, vec![b"1".to_vec(), b"2".to_vec(), b"3".to_vec()]);

        // Test negative indices
        let range = store.lrange("mylist", -2, -1).unwrap();
        assert_eq!(range, vec![b"3".to_vec(), b"4".to_vec()]);

        // Test entire range
        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range.len(), 5);
    }

    #[test]
    fn test_lindex() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"zero".to_vec(), b"one".to_vec(), b"two".to_vec()],
                false,
            )
            .unwrap();

        // Test positive index
        let value = store.lindex("mylist", 1).unwrap();
        assert_eq!(value, b"one".to_vec());

        // Test negative index
        let value = store.lindex("mylist", -1).unwrap();
        assert_eq!(value, b"two".to_vec());

        // Test out of range
        let result = store.lindex("mylist", 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_lset() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"zero".to_vec(), b"one".to_vec(), b"two".to_vec()],
                false,
            )
            .unwrap();

        // Test set at index
        let result = store.lset("mylist", 1, b"ONE".to_vec());
        assert!(result.is_ok());

        let value = store.lindex("mylist", 1).unwrap();
        assert_eq!(value, b"ONE".to_vec());

        // Test set with negative index
        let result = store.lset("mylist", -1, b"TWO".to_vec());
        assert!(result.is_ok());

        let value = store.lindex("mylist", -1).unwrap();
        assert_eq!(value, b"TWO".to_vec());
    }

    #[test]
    fn test_ltrim() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![
                    b"0".to_vec(),
                    b"1".to_vec(),
                    b"2".to_vec(),
                    b"3".to_vec(),
                    b"4".to_vec(),
                ],
                false,
            )
            .unwrap();

        // Trim to keep only [1, 3]
        store.ltrim("mylist", 1, 3).unwrap();

        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range, vec![b"1".to_vec(), b"2".to_vec(), b"3".to_vec()]);
    }

    #[test]
    fn test_lrem() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![
                    b"a".to_vec(),
                    b"b".to_vec(),
                    b"a".to_vec(),
                    b"c".to_vec(),
                    b"a".to_vec(),
                ],
                false,
            )
            .unwrap();

        // Remove 2 occurrences of "a" from head
        let removed = store.lrem("mylist", 2, b"a".to_vec()).unwrap();
        assert_eq!(removed, 2);

        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range, vec![b"b".to_vec(), b"c".to_vec(), b"a".to_vec()]);
    }

    #[test]
    fn test_linsert() {
        let store = ListStore::new();
        store
            .rpush("mylist", vec![b"a".to_vec(), b"c".to_vec()], false)
            .unwrap();

        // Insert before "c"
        let len = store
            .linsert("mylist", true, b"c".to_vec(), b"b".to_vec())
            .unwrap();
        assert_eq!(len, 3);

        let range = store.lrange("mylist", 0, -1).unwrap();
        assert_eq!(range, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    }

    #[test]
    fn test_lpos() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                false,
            )
            .unwrap();

        let pos = store.lpos("mylist", b"b".to_vec()).unwrap();
        assert_eq!(pos, Some(1));

        let pos = store.lpos("mylist", b"z".to_vec()).unwrap();
        assert_eq!(pos, None);
    }

    #[test]
    fn test_rpoplpush() {
        let store = ListStore::new();
        store
            .rpush(
                "source",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                false,
            )
            .unwrap();

        let value = store.rpoplpush("source", "dest").unwrap();
        assert_eq!(value, b"c".to_vec());

        let source_range = store.lrange("source", 0, -1).unwrap();
        assert_eq!(source_range, vec![b"a".to_vec(), b"b".to_vec()]);

        let dest_range = store.lrange("dest", 0, -1).unwrap();
        assert_eq!(dest_range, vec![b"c".to_vec()]);
    }

    #[test]
    fn test_llen() {
        let store = ListStore::new();
        store
            .rpush(
                "mylist",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                false,
            )
            .unwrap();

        let len = store.llen("mylist").unwrap();
        assert_eq!(len, 3);
    }

    #[test]
    fn test_lpushx_exists() {
        let store = ListStore::new();

        // LPUSHX on non-existent key should return 0
        let result = store.lpush("mylist", vec![b"a".to_vec()], true).unwrap();
        assert_eq!(result, 0);

        // Create the list
        store.lpush("mylist", vec![b"a".to_vec()], false).unwrap();

        // Now LPUSHX should work
        let result = store.lpush("mylist", vec![b"b".to_vec()], true).unwrap();
        assert_eq!(result, 2);
    }

    #[tokio::test]
    async fn test_blpop_immediate() {
        let store = ListStore::new();
        store
            .rpush("mylist", vec![b"value".to_vec()], false)
            .unwrap();

        let (key, value) = store
            .blpop(vec!["mylist".to_string()], Some(1))
            .await
            .unwrap();
        assert_eq!(key, "mylist");
        assert_eq!(value, b"value".to_vec());
    }

    #[tokio::test]
    async fn test_blpop_timeout() {
        let store = ListStore::new();

        let result = store.blpop(vec!["mylist".to_string()], Some(1)).await;
        assert!(matches!(result, Err(SynapError::Timeout)));
    }

    #[tokio::test]
    async fn test_brpoplpush_immediate() {
        let store = ListStore::new();
        store
            .rpush("source", vec![b"value".to_vec()], false)
            .unwrap();

        let value = store.brpoplpush("source", "dest", Some(1)).await.unwrap();
        assert_eq!(value, b"value".to_vec());

        let dest_range = store.lrange("dest", 0, -1).unwrap();
        assert_eq!(dest_range, vec![b"value".to_vec()]);
    }

    #[tokio::test]
    async fn test_brpoplpush_timeout() {
        let store = ListStore::new();

        let result = store.brpoplpush("source", "dest", Some(1)).await;
        assert!(matches!(result, Err(SynapError::Timeout)));
    }

    #[test]
    fn test_index_set_and_range_edge_cases() {
        let store = ListStore::new();
        store
            .rpush(
                "l",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec(), b"d".to_vec()],
                false,
            )
            .unwrap();

        // Negative and out-of-range indices.
        assert_eq!(store.lindex("l", -1).unwrap(), b"d".to_vec());
        assert!(store.lindex("l", 99).is_err());
        assert_eq!(
            store.lrange("l", -2, -1).unwrap(),
            vec![b"c".to_vec(), b"d".to_vec()]
        );
        // start > stop yields an empty range.
        assert!(store.lrange("l", 3, 1).unwrap().is_empty());

        store.lset("l", 0, b"A".to_vec()).unwrap();
        assert_eq!(store.lindex("l", 0).unwrap(), b"A".to_vec());
        assert!(store.lset("l", 99, b"z".to_vec()).is_err());

        assert_eq!(store.llen("l").unwrap(), 4);
        assert_eq!(store.lpos("l", b"c".to_vec()).unwrap(), Some(2));
        assert_eq!(store.lpos("l", b"zzz".to_vec()).unwrap(), None);
    }

    #[test]
    fn test_trim_rem_insert() {
        let store = ListStore::new();
        store
            .rpush(
                "l",
                vec![
                    b"a".to_vec(),
                    b"b".to_vec(),
                    b"a".to_vec(),
                    b"c".to_vec(),
                    b"a".to_vec(),
                ],
                false,
            )
            .unwrap();

        // Remove 2 occurrences of "a" from head.
        assert_eq!(store.lrem("l", 2, b"a".to_vec()).unwrap(), 2);
        // Remove 1 occurrence from tail.
        assert_eq!(store.lrem("l", -1, b"a".to_vec()).unwrap(), 1);
        assert_eq!(
            store.lrange("l", 0, -1).unwrap(),
            vec![b"b".to_vec(), b"c".to_vec()]
        );

        store
            .linsert("l", true, b"c".to_vec(), b"x".to_vec())
            .unwrap();
        assert_eq!(
            store.lrange("l", 0, -1).unwrap(),
            vec![b"b".to_vec(), b"x".to_vec(), b"c".to_vec()]
        );

        store.ltrim("l", 1, 2).unwrap();
        assert_eq!(
            store.lrange("l", 0, -1).unwrap(),
            vec![b"x".to_vec(), b"c".to_vec()]
        );
    }

    #[test]
    fn test_pop_count_and_only_if_exists() {
        let store = ListStore::new();
        // only_if_exists on a missing key does nothing.
        assert_eq!(
            store.lpush("missing", vec![b"a".to_vec()], true).unwrap(),
            0
        );
        assert!(!store.exists("missing"));

        store
            .rpush(
                "l",
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                false,
            )
            .unwrap();
        assert_eq!(
            store.lpop("l", Some(2)).unwrap(),
            vec![b"a".to_vec(), b"b".to_vec()]
        );
        assert_eq!(store.rpop("l", Some(5)).unwrap(), vec![b"c".to_vec()]);
        // Empty list is removed.
        assert!(!store.exists("l"));
    }

    #[test]
    fn test_stats_delete_and_memory() {
        let store = ListStore::new();
        store.rpush("a", vec![b"1".to_vec()], false).unwrap();
        store
            .rpush("b", vec![b"2".to_vec(), b"3".to_vec()], false)
            .unwrap();

        let stats = store.stats();
        assert_eq!(stats.total_lists, 2);
        assert_eq!(stats.total_elements, 3);
        store.refresh_memory();

        assert!(store.delete("a").unwrap());
        assert!(!store.delete("a").unwrap());
        assert!(store.exists("b"));
    }
}
