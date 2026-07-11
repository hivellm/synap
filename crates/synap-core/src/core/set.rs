//! Set data structure implementation for Synap
//!
//! Provides Redis-compatible set operations (SADD, SREM, SISMEMBER, SINTER, SUNION, SDIFF, etc.)
//! Storage: HashSet for O(1) member operations and efficient set algebra
//!
//! # Performance Targets
//! - SADD: <100µs p99 latency
//! - SISMEMBER: <50µs p99 latency
//! - SMEMBERS (1K items): <1ms p99 latency
//! - SINTER (2 sets, 10K items): <5ms p99 latency
//!
//! # Architecture
//! ```text
//! SetStore
//!   ├─ 64 shards (Arc<RwLock<HashMap<key, SetValue>>>)
//!   └─ TTL applies to entire set
//! ```

use super::error::{Result, SynapError};
use ahash::RandomState as AHashState;
use parking_lot::RwLock;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const SHARD_COUNT: usize = 64;

/// Packed-encoding thresholds for small sets (mirrors the list encoding; the
/// Redis intset/listpack analogue).
const MAX_PACKED_SET_ENTRIES: usize = 128;
const MAX_PACKED_SET_ELEM: usize = 64;

/// Storage representation for a set (phase13 contiguous encoding).
///
/// Small sets are stored **packed**: `[u32 LE len][bytes]…` unique entries in
/// one contiguous buffer — no `HashSet` allocation per small key, membership by
/// bounded scan (≤128 entries). Past the thresholds the set upgrades one-way to
/// the `ahash` `HashSet` representation.
#[derive(Debug, Clone)]
enum SetRepr {
    Packed { buf: Vec<u8>, count: usize },
    Hash(HashSet<Vec<u8>, AHashState>),
}

impl SetRepr {
    /// Iterate the packed entries as byte slices.
    fn packed_iter(buf: &[u8]) -> impl Iterator<Item = &[u8]> {
        let mut pos = 0usize;
        std::iter::from_fn(move || {
            if pos + 4 > buf.len() {
                return None;
            }
            let len =
                u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
            let start = pos + 4;
            pos = start + len;
            buf.get(start..start + len)
        })
    }

    fn append_entry(buf: &mut Vec<u8>, value: &[u8]) {
        buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
        buf.extend_from_slice(value);
    }
}

/// Serialize the representation as the logical member sequence — the same
/// encoding the previous `HashSet` field produced, so snapshots stay
/// byte-compatible in both directions.
mod set_repr_as_seq {
    use super::SetRepr;
    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(repr: &SetRepr, s: S) -> Result<S::Ok, S::Error> {
        match repr {
            SetRepr::Packed { buf, count } => {
                let mut seq = s.serialize_seq(Some(*count))?;
                for e in SetRepr::packed_iter(buf) {
                    seq.serialize_element(e)?;
                }
                seq.end()
            }
            SetRepr::Hash(h) => {
                let mut seq = s.serialize_seq(Some(h.len()))?;
                for e in h {
                    seq.serialize_element(e)?;
                }
                seq.end()
            }
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SetRepr, D::Error> {
        let members = Vec::<Vec<u8>>::deserialize(d)?;
        Ok(super::SetValue::repr_from_members(members))
    }
}

/// Set value stored in a single key
/// Contains unique members
///
/// Small sets use a contiguous packed encoding; large ones an `ahash`-keyed
/// `HashSet` (phase13 contiguous encoding + write-scalability).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetValue {
    /// Unique members
    #[serde(rename = "members", with = "set_repr_as_seq")]
    repr: SetRepr,
    /// TTL for entire set
    pub ttl_secs: Option<u64>,
    /// Creation timestamp
    pub created_at: u64,
    /// Last modification timestamp
    pub updated_at: u64,
}

impl SetValue {
    /// Create new set value
    pub fn new(ttl_secs: Option<u64>) -> Self {
        let now = Self::current_timestamp();
        Self {
            repr: SetRepr::Packed {
                buf: Vec::new(),
                count: 0,
            },
            ttl_secs,
            created_at: now,
            updated_at: now,
        }
    }

    /// Build the right representation from a known (assumed-unique) member
    /// sequence (snapshot recovery / deserialization / *STORE results).
    fn repr_from_members(members: Vec<Vec<u8>>) -> SetRepr {
        let fits = members.len() <= MAX_PACKED_SET_ENTRIES
            && members.iter().all(|m| m.len() <= MAX_PACKED_SET_ELEM);
        if fits {
            let mut buf = Vec::new();
            let count = members.len();
            for m in &members {
                SetRepr::append_entry(&mut buf, m);
            }
            SetRepr::Packed { buf, count }
        } else {
            SetRepr::Hash(members.into_iter().collect())
        }
    }

    /// Upgrade to the `HashSet` representation (one-way) and return it.
    fn ensure_hash(&mut self) -> &mut HashSet<Vec<u8>, AHashState> {
        if let SetRepr::Packed { buf, .. } = &self.repr {
            let members: HashSet<Vec<u8>, AHashState> =
                SetRepr::packed_iter(buf).map(|m| m.to_vec()).collect();
            self.repr = SetRepr::Hash(members);
        }
        match &mut self.repr {
            SetRepr::Hash(h) => h,
            SetRepr::Packed { .. } => unreachable!("just upgraded"),
        }
    }

    /// Consume into the logical member sequence (snapshot recovery).
    pub fn into_members(self) -> Vec<Vec<u8>> {
        match self.repr {
            SetRepr::Packed { buf, .. } => SetRepr::packed_iter(&buf).map(|m| m.to_vec()).collect(),
            SetRepr::Hash(h) => h.into_iter().collect(),
        }
    }

    /// Collect the members into an owned `HashSet` (set-algebra base).
    pub fn to_hash_set(&self) -> HashSet<Vec<u8>, AHashState> {
        match &self.repr {
            SetRepr::Packed { buf, .. } => SetRepr::packed_iter(buf).map(|m| m.to_vec()).collect(),
            SetRepr::Hash(h) => h.clone(),
        }
    }

    /// Replace the whole membership (S*STORE results).
    pub fn replace_members(&mut self, members: Vec<Vec<u8>>) {
        self.updated_at = Self::current_timestamp();
        self.repr = Self::repr_from_members(members);
    }

    /// Total payload bytes across members (memory accounting).
    pub fn member_bytes(&self) -> usize {
        match &self.repr {
            SetRepr::Packed { buf, count } => buf.len().saturating_sub(4 * count),
            SetRepr::Hash(h) => h.iter().map(Vec::len).sum(),
        }
    }

    /// Get current Unix timestamp
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Check if set has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_secs {
            let now = Self::current_timestamp();
            now >= self.created_at + ttl
        } else {
            false
        }
    }

    /// Get number of members
    pub fn len(&self) -> usize {
        match &self.repr {
            SetRepr::Packed { count, .. } => *count,
            SetRepr::Hash(h) => h.len(),
        }
    }

    /// Check if set is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Add one member. Returns true when it was not already present.
    pub fn insert_one(&mut self, member: Vec<u8>) -> bool {
        self.updated_at = Self::current_timestamp();
        match &mut self.repr {
            SetRepr::Packed { buf, count } => {
                if SetRepr::packed_iter(buf).any(|m| m == member.as_slice()) {
                    return false;
                }
                if *count < MAX_PACKED_SET_ENTRIES && member.len() <= MAX_PACKED_SET_ELEM {
                    SetRepr::append_entry(buf, &member);
                    *count += 1;
                    true
                } else {
                    self.ensure_hash().insert(member)
                }
            }
            SetRepr::Hash(h) => h.insert(member),
        }
    }

    /// Add member(s), returns number of members added
    pub fn add(&mut self, members: Vec<Vec<u8>>) -> usize {
        members
            .into_iter()
            .filter(|m| self.insert_one(m.clone()))
            .count()
    }

    /// Remove one member. Returns true when it was present.
    pub fn remove_one(&mut self, member: &[u8]) -> bool {
        self.updated_at = Self::current_timestamp();
        match &mut self.repr {
            SetRepr::Packed { buf, count } => {
                let mut pos = 0usize;
                while pos + 4 <= buf.len() {
                    let len =
                        u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]])
                            as usize;
                    let start = pos + 4;
                    if buf.get(start..start + len) == Some(member) {
                        buf.drain(pos..start + len);
                        *count -= 1;
                        return true;
                    }
                    pos = start + len;
                }
                false
            }
            SetRepr::Hash(h) => h.remove(member),
        }
    }

    /// Remove member(s), returns number of members removed
    pub fn remove(&mut self, members: &[Vec<u8>]) -> usize {
        members.iter().filter(|m| self.remove_one(m)).count()
    }

    /// Check if member exists
    pub fn is_member(&self, member: &[u8]) -> bool {
        match &self.repr {
            SetRepr::Packed { buf, .. } => SetRepr::packed_iter(buf).any(|m| m == member),
            SetRepr::Hash(h) => h.contains(member),
        }
    }

    /// Get all members
    pub fn members(&self) -> Vec<Vec<u8>> {
        match &self.repr {
            SetRepr::Packed { buf, .. } => SetRepr::packed_iter(buf).map(|m| m.to_vec()).collect(),
            SetRepr::Hash(h) => h.iter().cloned().collect(),
        }
    }

    /// Pop random member
    pub fn pop(&mut self, count: usize) -> Vec<Vec<u8>> {
        self.updated_at = Self::current_timestamp();
        let taken: Vec<Vec<u8>> = self.members().into_iter().take(count).collect();
        for m in &taken {
            self.remove_one(m);
        }
        taken
    }

    /// Get random member(s) without removing
    pub fn random_members(&self, count: usize) -> Vec<Vec<u8>> {
        let mut members = self.members();
        if count >= members.len() {
            members
        } else {
            let mut rng = rand::rng();
            members.shuffle(&mut rng);
            members.into_iter().take(count).collect()
        }
    }
}

/// Statistics for set operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SetStats {
    pub total_sets: usize,
    pub total_members: usize,
    pub sadd_count: u64,
    pub srem_count: u64,
    pub sismember_count: u64,
    pub smembers_count: u64,
    pub scard_count: u64,
    pub spop_count: u64,
    pub srandmember_count: u64,
    pub smove_count: u64,
    pub sinter_count: u64,
    pub sunion_count: u64,
    pub sdiff_count: u64,
}

/// Lock-free per-op counters (phase12) — bumping a counter no longer takes a
/// global `RwLock`. Structural totals are recomputed on demand in
/// [`SetStore::stats`], so only the counters live here.
#[derive(Default)]
struct AtomicSetStats {
    sadd_count: AtomicU64,
    srem_count: AtomicU64,
    sismember_count: AtomicU64,
    smembers_count: AtomicU64,
    scard_count: AtomicU64,
    spop_count: AtomicU64,
    srandmember_count: AtomicU64,
    smove_count: AtomicU64,
    sinter_count: AtomicU64,
    sunion_count: AtomicU64,
    sdiff_count: AtomicU64,
}

impl AtomicSetStats {
    fn snapshot(&self) -> SetStats {
        SetStats {
            total_sets: 0,
            total_members: 0,
            sadd_count: self.sadd_count.load(Ordering::Relaxed),
            srem_count: self.srem_count.load(Ordering::Relaxed),
            sismember_count: self.sismember_count.load(Ordering::Relaxed),
            smembers_count: self.smembers_count.load(Ordering::Relaxed),
            scard_count: self.scard_count.load(Ordering::Relaxed),
            spop_count: self.spop_count.load(Ordering::Relaxed),
            srandmember_count: self.srandmember_count.load(Ordering::Relaxed),
            smove_count: self.smove_count.load(Ordering::Relaxed),
            sinter_count: self.sinter_count.load(Ordering::Relaxed),
            sunion_count: self.sunion_count.load(Ordering::Relaxed),
            sdiff_count: self.sdiff_count.load(Ordering::Relaxed),
        }
    }
}

/// Sharded set store with 64-way concurrency
pub struct SetStore {
    shards: Vec<Arc<RwLock<HashMap<String, SetValue, AHashState>>>>,
    stats: Arc<AtomicSetStats>,
    /// Shared cross-datatype memory budget (audit M-018).
    mem: Option<crate::core::GlobalMemory>,
    mem_bytes: Arc<std::sync::atomic::AtomicI64>,
    /// Optional keyspace-notification publisher (Redis `notify-keyspace-events`).
    keyspace_notifier: Option<Arc<crate::core::KeyspaceNotifier>>,
}

impl Default for SetStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SetStore {
    /// Dump every set (key -> SetValue) across all shards, for snapshotting.
    pub fn dump(&self) -> HashMap<String, SetValue> {
        let mut out = HashMap::new();
        for shard in self.shards.iter() {
            let guard = shard.read();
            for (key, v) in guard.iter() {
                out.insert(key.clone(), v.clone());
            }
        }
        out
    }

    /// Create new set store
    pub fn new() -> Self {
        let mut shards = Vec::with_capacity(SHARD_COUNT);
        for _ in 0..SHARD_COUNT {
            shards.push(Arc::new(RwLock::new(HashMap::default())));
        }

        Self {
            shards,
            stats: Arc::new(AtomicSetStats::default()),
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

    /// Attach a keyspace-notification publisher so set mutations publish
    /// `s`-class events. A no-op when `notifier` is `None`.
    pub fn with_keyspace_notifier(
        mut self,
        notifier: Option<Arc<crate::core::KeyspaceNotifier>>,
    ) -> Self {
        self.keyspace_notifier = notifier;
        self
    }

    /// Publish a set keyspace notification for `key` if a notifier is attached.
    #[inline]
    fn notify_keyspace(&self, event: &str, key: &str) {
        if let Some(ref n) = self.keyspace_notifier {
            n.notify(crate::core::EventClass::Set, event, key);
        }
    }

    /// Total payload bytes currently held (keys + members across shards).
    pub fn memory_bytes(&self) -> usize {
        let mut total = 0usize;
        for shard in self.shards.iter() {
            for (key, v) in shard.read().iter() {
                total += key.len();
                total += v.member_bytes();
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
    fn shard(&self, key: &str) -> &Arc<RwLock<HashMap<String, SetValue, AHashState>>> {
        &self.shards[self.shard_index(key)]
    }

    /// SADD - Add member(s) to set
    pub fn sadd(&self, key: &str, members: Vec<Vec<u8>>) -> Result<usize> {
        self.check_admit(members.iter().map(|m| m.len()).sum())?;
        let shard = self.shard(key);
        let mut map = shard.write();

        // Fast path: existing, live set — single `get_mut`, no `key.to_string()`.
        // Only allocate the key (and hit `entry`) when the set is missing or
        // expired (the cold path).
        let added = match map.get_mut(key) {
            Some(set) if !set.is_expired() => set.add(members),
            _ => {
                let mut set = SetValue::new(None);
                let added = set.add(members);
                map.insert(key.to_string(), set);
                added
            }
        };
        self.stats.sadd_count.fetch_add(1, Ordering::Relaxed);

        drop(map);
        if added > 0 {
            self.notify_keyspace("sadd", key);
        }
        Ok(added)
    }

    /// SREM - Remove member(s) from set
    pub fn srem(&self, key: &str, members: Vec<Vec<u8>>) -> Result<usize> {
        let shard = self.shard(key);
        let mut map = shard.write();

        let set = map.get_mut(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if set.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        let removed = set.remove(&members);
        self.stats.srem_count.fetch_add(1, Ordering::Relaxed);

        // Remove empty sets
        if set.is_empty() {
            map.remove(key);
        }

        drop(map);
        if removed > 0 {
            self.notify_keyspace("srem", key);
        }
        Ok(removed)
    }

    /// SISMEMBER - Check if member exists in set
    pub fn sismember(&self, key: &str, member: Vec<u8>) -> Result<bool> {
        let shard = self.shard(key);
        let map = shard.read();

        let set = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if set.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        self.stats.sismember_count.fetch_add(1, Ordering::Relaxed);
        Ok(set.is_member(&member))
    }

    /// SMEMBERS - Get all members of set
    pub fn smembers(&self, key: &str) -> Result<Vec<Vec<u8>>> {
        let shard = self.shard(key);
        let map = shard.read();

        let set = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if set.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        self.stats.smembers_count.fetch_add(1, Ordering::Relaxed);
        Ok(set.members())
    }

    /// SSCAN - cursor-based incremental scan of a set's members.
    ///
    /// `cursor` is an offset into a stably-sorted snapshot of the members; returns
    /// the next cursor (0 when complete) and the matched members within the
    /// window. `pattern` is an optional glob over members; `count` bounds the
    /// window size (min 1). A missing key scans as empty.
    pub fn sscan(
        &self,
        key: &str,
        cursor: u64,
        pattern: Option<&str>,
        count: usize,
    ) -> Result<(u64, Vec<Vec<u8>>)> {
        let mut members = self.smembers(key).unwrap_or_default();
        members.sort();
        let total = members.len();
        let start = (cursor as usize).min(total);
        let end = start.saturating_add(count.max(1)).min(total);
        let items = members[start..end]
            .iter()
            .filter(|m| {
                pattern.is_none_or(|p| crate::core::glob_match(p, &String::from_utf8_lossy(m)))
            })
            .cloned()
            .collect();
        let next = if end < total { end as u64 } else { 0 };
        Ok((next, items))
    }

    /// SCARD - Get set cardinality (size)
    pub fn scard(&self, key: &str) -> Result<usize> {
        let shard = self.shard(key);
        let map = shard.read();

        let set = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if set.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        self.stats.scard_count.fetch_add(1, Ordering::Relaxed);
        Ok(set.len())
    }

    /// SPOP - Remove and return random member(s)
    pub fn spop(&self, key: &str, count: Option<usize>) -> Result<Vec<Vec<u8>>> {
        let shard = self.shard(key);
        let mut map = shard.write();

        let set = map.get_mut(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if set.is_expired() {
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        let count = count.unwrap_or(1);
        let result = set.pop(count);
        self.stats.spop_count.fetch_add(1, Ordering::Relaxed);

        // Remove empty sets
        if set.is_empty() {
            map.remove(key);
        }

        Ok(result)
    }

    /// SRANDMEMBER - Get random member(s) without removing
    pub fn srandmember(&self, key: &str, count: Option<usize>) -> Result<Vec<Vec<u8>>> {
        let shard = self.shard(key);
        let map = shard.read();

        let set = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if set.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        let count = count.unwrap_or(1);
        self.stats.srandmember_count.fetch_add(1, Ordering::Relaxed);
        Ok(set.random_members(count))
    }

    /// SMOVE - Move member from source to destination
    pub fn smove(&self, source: &str, destination: &str, member: Vec<u8>) -> Result<bool> {
        let source_idx = self.shard_index(source);
        let dest_idx = self.shard_index(destination);

        // Handle same shard case
        if source_idx == dest_idx {
            let shard = &self.shards[source_idx];
            let mut map = shard.write();

            // Remove from source
            let source_set = map.get_mut(source).ok_or(SynapError::NotFound)?;

            if source_set.is_expired() {
                map.remove(source);
                return Err(SynapError::KeyExpired);
            }

            if !source_set.remove_one(&member) {
                return Ok(false); // Member not in source
            }

            // Remove empty source
            if source_set.is_empty() {
                map.remove(source);
            }

            // Add to destination
            if let Some(existing_set) = map.get(destination)
                && existing_set.is_expired()
            {
                map.remove(destination);
            }

            let dest_set = map
                .entry(destination.to_string())
                .or_insert_with(|| SetValue::new(None));

            dest_set.insert_one(member);
            dest_set.updated_at = SetValue::current_timestamp();

            self.stats.smove_count.fetch_add(1, Ordering::Relaxed);
            return Ok(true);
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

        let (source_map, dest_map) = if source_idx < dest_idx {
            (&mut *first_map, &mut *second_map)
        } else {
            (&mut *second_map, &mut *first_map)
        };

        // Remove from source
        let source_set = source_map.get_mut(source).ok_or(SynapError::NotFound)?;

        if source_set.is_expired() {
            source_map.remove(source);
            return Err(SynapError::KeyExpired);
        }

        if !source_set.remove_one(&member) {
            return Ok(false);
        }

        // Remove empty source
        if source_set.is_empty() {
            source_map.remove(source);
        }

        // Add to destination
        if let Some(existing_set) = dest_map.get(destination)
            && existing_set.is_expired()
        {
            dest_map.remove(destination);
        }

        let dest_set = dest_map
            .entry(destination.to_string())
            .or_insert_with(|| SetValue::new(None));

        dest_set.insert_one(member);
        dest_set.updated_at = SetValue::current_timestamp();

        self.stats.smove_count.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }

    /// SINTER - Intersection of multiple sets
    pub fn sinter(&self, keys: &[String]) -> Result<Vec<Vec<u8>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // Read all sets
        let sets: Vec<_> = keys
            .iter()
            .map(|key| {
                let shard = self.shard(key);
                let map = shard.read();
                map.get(key)
                    .filter(|s| !s.is_expired())
                    .map(|s| s.to_hash_set())
            })
            .collect();

        // If any set is missing or empty, intersection is empty
        let mut result = sets.first().and_then(|opt| opt.clone()).unwrap_or_default();

        for set_opt in sets.iter().skip(1) {
            if let Some(set) = set_opt {
                result.retain(|member| set.contains(member));
            } else {
                return Ok(Vec::new()); // Missing set means empty intersection
            }
        }

        self.stats.sinter_count.fetch_add(1, Ordering::Relaxed);
        Ok(result.into_iter().collect())
    }

    /// SUNION - Union of multiple sets
    pub fn sunion(&self, keys: &[String]) -> Result<Vec<Vec<u8>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut result = HashSet::new();

        for key in keys {
            let shard = self.shard(key);
            let map = shard.read();
            if let Some(set) = map.get(key)
                && !set.is_expired()
            {
                result.extend(set.members());
            }
        }

        self.stats.sunion_count.fetch_add(1, Ordering::Relaxed);
        Ok(result.into_iter().collect())
    }

    /// SDIFF - Difference of sets (first set minus all others)
    pub fn sdiff(&self, keys: &[String]) -> Result<Vec<Vec<u8>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // Start with first set
        let first_key = &keys[0];
        let first_shard = self.shard(first_key);
        let first_map = first_shard.read();

        let mut result = first_map
            .get(first_key)
            .filter(|s| !s.is_expired())
            .map(|s| s.to_hash_set())
            .unwrap_or_default();

        // Remove members from other sets
        for key in keys.iter().skip(1) {
            let shard = self.shard(key);
            let map = shard.read();
            if let Some(set) = map.get(key)
                && !set.is_expired()
            {
                result.retain(|member| !set.is_member(member));
            }
        }

        self.stats.sdiff_count.fetch_add(1, Ordering::Relaxed);
        Ok(result.into_iter().collect())
    }

    /// SINTERSTORE - Store intersection in destination
    pub fn sinterstore(&self, destination: &str, keys: &[String]) -> Result<usize> {
        let members = self.sinter(keys)?;
        let count = members.len();

        // Store result in destination
        let shard = self.shard(destination);
        let mut map = shard.write();

        let mut set = SetValue::new(None);
        set.replace_members(members.into_iter().collect());
        map.insert(destination.to_string(), set);

        Ok(count)
    }

    /// SUNIONSTORE - Store union in destination
    pub fn sunionstore(&self, destination: &str, keys: &[String]) -> Result<usize> {
        let members = self.sunion(keys)?;
        let count = members.len();

        // Store result in destination
        let shard = self.shard(destination);
        let mut map = shard.write();

        let mut set = SetValue::new(None);
        set.replace_members(members.into_iter().collect());
        map.insert(destination.to_string(), set);

        Ok(count)
    }

    /// SDIFFSTORE - Store difference in destination
    pub fn sdiffstore(&self, destination: &str, keys: &[String]) -> Result<usize> {
        let members = self.sdiff(keys)?;
        let count = members.len();

        // Store result in destination
        let shard = self.shard(destination);
        let mut map = shard.write();

        let mut set = SetValue::new(None);
        set.replace_members(members.into_iter().collect());
        map.insert(destination.to_string(), set);

        Ok(count)
    }

    /// Get statistics
    pub fn stats(&self) -> SetStats {
        let mut stats = self.stats.snapshot();

        // Count total sets and members
        let mut total_sets = 0;
        let mut total_members = 0;

        for shard in &self.shards {
            let map = shard.read();
            total_sets += map.len();
            for set in map.values() {
                if !set.is_expired() {
                    total_members += set.len();
                }
            }
        }

        stats.total_sets = total_sets;
        stats.total_members = total_members;

        stats
    }

    /// Delete a set
    pub fn delete(&self, key: &str) -> Result<bool> {
        let shard = self.shard(key);
        let mut map = shard.write();
        Ok(map.remove(key).is_some())
    }

    /// Check if set exists
    pub fn exists(&self, key: &str) -> bool {
        let shard = self.shard(key);
        let map = shard.read();
        if let Some(set) = map.get(key) {
            !set.is_expired()
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sadd_sismember() {
        let store = SetStore::new();

        // Add members
        let added = store
            .sadd("myset", vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .unwrap();
        assert_eq!(added, 3);

        // Test membership
        assert!(store.sismember("myset", b"a".to_vec()).unwrap());
        assert!(store.sismember("myset", b"b".to_vec()).unwrap());
        assert!(!store.sismember("myset", b"z".to_vec()).unwrap());

        // Add duplicate (should return 0)
        let added2 = store.sadd("myset", vec![b"a".to_vec()]).unwrap();
        assert_eq!(added2, 0);
    }

    #[test]
    fn test_srem() {
        let store = SetStore::new();
        store
            .sadd("myset", vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .unwrap();

        let removed = store
            .srem("myset", vec![b"a".to_vec(), b"b".to_vec()])
            .unwrap();
        assert_eq!(removed, 2);

        assert!(!store.sismember("myset", b"a".to_vec()).unwrap());
        assert!(store.sismember("myset", b"c".to_vec()).unwrap());
    }

    #[test]
    fn test_smembers() {
        let store = SetStore::new();
        store
            .sadd("myset", vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .unwrap();

        let members = store.smembers("myset").unwrap();
        assert_eq!(members.len(), 3);
        assert!(members.contains(&b"a".to_vec()));
        assert!(members.contains(&b"b".to_vec()));
        assert!(members.contains(&b"c".to_vec()));
    }

    #[test]
    fn test_scard() {
        let store = SetStore::new();
        store
            .sadd("myset", vec![b"a".to_vec(), b"b".to_vec()])
            .unwrap();

        let count = store.scard("myset").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_spop() {
        let store = SetStore::new();
        store
            .sadd("myset", vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .unwrap();

        let popped = store.spop("myset", Some(2)).unwrap();
        assert_eq!(popped.len(), 2);

        let count = store.scard("myset").unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_srandmember() {
        let store = SetStore::new();
        store
            .sadd("myset", vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .unwrap();

        let random = store.srandmember("myset", Some(2)).unwrap();
        assert_eq!(random.len(), 2);

        // Set should still have all 3 members
        let count = store.scard("myset").unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_smove() {
        let store = SetStore::new();
        store
            .sadd("source", vec![b"a".to_vec(), b"b".to_vec()])
            .unwrap();

        let moved = store.smove("source", "dest", b"a".to_vec()).unwrap();
        assert!(moved);

        assert!(!store.sismember("source", b"a".to_vec()).unwrap());
        assert!(store.sismember("dest", b"a".to_vec()).unwrap());
    }

    #[test]
    fn test_sinter() {
        let store = SetStore::new();
        store
            .sadd("set1", vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .unwrap();
        store
            .sadd("set2", vec![b"b".to_vec(), b"c".to_vec(), b"d".to_vec()])
            .unwrap();

        let inter = store
            .sinter(&[String::from("set1"), String::from("set2")])
            .unwrap();
        assert_eq!(inter.len(), 2);
        assert!(inter.contains(&b"b".to_vec()));
        assert!(inter.contains(&b"c".to_vec()));
    }

    #[test]
    fn test_sunion() {
        let store = SetStore::new();
        store
            .sadd("set1", vec![b"a".to_vec(), b"b".to_vec()])
            .unwrap();
        store
            .sadd("set2", vec![b"b".to_vec(), b"c".to_vec()])
            .unwrap();

        let union = store
            .sunion(&[String::from("set1"), String::from("set2")])
            .unwrap();
        assert_eq!(union.len(), 3);
        assert!(union.contains(&b"a".to_vec()));
        assert!(union.contains(&b"b".to_vec()));
        assert!(union.contains(&b"c".to_vec()));
    }

    #[test]
    fn test_sdiff() {
        let store = SetStore::new();
        store
            .sadd("set1", vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .unwrap();
        store.sadd("set2", vec![b"b".to_vec()]).unwrap();

        let diff = store
            .sdiff(&[String::from("set1"), String::from("set2")])
            .unwrap();
        assert_eq!(diff.len(), 2);
        assert!(diff.contains(&b"a".to_vec()));
        assert!(diff.contains(&b"c".to_vec()));
    }

    #[test]
    fn test_sinterstore() {
        let store = SetStore::new();
        store
            .sadd("set1", vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .unwrap();
        store
            .sadd("set2", vec![b"b".to_vec(), b"c".to_vec(), b"d".to_vec()])
            .unwrap();

        let count = store
            .sinterstore("result", &[String::from("set1"), String::from("set2")])
            .unwrap();
        assert_eq!(count, 2);

        let members = store.smembers("result").unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.contains(&b"b".to_vec()));
        assert!(members.contains(&b"c".to_vec()));
    }

    fn seeded() -> SetStore {
        let store = SetStore::new();
        store
            .sadd("s1", vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()])
            .unwrap();
        store
            .sadd("s2", vec![b"b".to_vec(), b"c".to_vec(), b"d".to_vec()])
            .unwrap();
        store
    }

    #[test]
    fn test_scard_spop_srandmember() {
        let store = seeded();
        assert_eq!(store.scard("s1").unwrap(), 3);

        let popped = store.spop("s1", Some(2)).unwrap();
        assert_eq!(popped.len(), 2);
        assert_eq!(store.scard("s1").unwrap(), 1);

        // SRANDMEMBER does not remove members.
        let rnd = store.srandmember("s2", Some(2)).unwrap();
        assert_eq!(rnd.len(), 2);
        assert_eq!(store.scard("s2").unwrap(), 3);
        // Count larger than cardinality returns all distinct members.
        let all = store.srandmember("s2", Some(10)).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_smove_between_sets() {
        let store = seeded();
        assert!(store.smove("s1", "s2", b"a".to_vec()).unwrap());
        assert!(!store.sismember("s1", b"a".to_vec()).unwrap());
        assert!(store.sismember("s2", b"a".to_vec()).unwrap());
        // Moving a non-member returns false.
        assert!(!store.smove("s1", "s2", b"nope".to_vec()).unwrap());
    }

    #[test]
    fn test_set_algebra_readonly() {
        let store = seeded();
        let keys = vec!["s1".to_string(), "s2".to_string()];

        let mut inter = store.sinter(&keys).unwrap();
        inter.sort();
        assert_eq!(inter, vec![b"b".to_vec(), b"c".to_vec()]);

        let union = store.sunion(&keys).unwrap();
        assert_eq!(union.len(), 4);

        let diff = store.sdiff(&keys).unwrap();
        assert_eq!(diff, vec![b"a".to_vec()]);
    }

    #[test]
    fn test_set_algebra_store_stats_delete() {
        let store = seeded();
        let keys = vec!["s1".to_string(), "s2".to_string()];

        assert_eq!(store.sinterstore("i", &keys).unwrap(), 2);
        assert_eq!(store.sunionstore("u", &keys).unwrap(), 4);
        assert_eq!(store.sdiffstore("d", &keys).unwrap(), 1);

        let stats = store.stats();
        assert!(stats.total_sets >= 2);
        store.refresh_memory();
        assert!(store.memory_bytes() > 0);

        assert!(store.delete("s1").unwrap());
        assert!(!store.exists("s1"));
        assert!(!store.delete("s1").unwrap());
    }

    #[test]
    fn test_value_helpers() {
        let mut v = SetValue::new(None);
        assert!(v.is_empty());
        assert_eq!(v.add(vec![b"x".to_vec(), b"y".to_vec(), b"x".to_vec()]), 2);
        assert_eq!(v.len(), 2);
        assert!(v.is_member(b"x"));
        assert_eq!(v.remove(&[b"x".to_vec()]), 1);
        let popped = v.pop(1);
        assert_eq!(popped.len(), 1);
        assert!(v.is_empty());
    }

    #[test]
    fn test_sscan_cursor_and_match() {
        let store = SetStore::new();
        store
            .sadd(
                "s",
                vec![
                    b"a1".to_vec(),
                    b"a2".to_vec(),
                    b"b1".to_vec(),
                    b"b2".to_vec(),
                ],
            )
            .unwrap();

        let mut seen = Vec::new();
        let mut cursor = 0u64;
        loop {
            let (next, items) = store.sscan("s", cursor, None, 2).unwrap();
            seen.extend(items);
            if next == 0 {
                break;
            }
            cursor = next;
        }
        seen.sort();
        assert_eq!(
            seen,
            vec![
                b"a1".to_vec(),
                b"a2".to_vec(),
                b"b1".to_vec(),
                b"b2".to_vec()
            ]
        );

        let (_c, items) = store.sscan("s", 0, Some("a*"), 100).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(store.sscan("missing", 0, None, 10).unwrap(), (0, vec![]));
    }
}
