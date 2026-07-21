//! Sharded sorted-set store over `SortedSetValue`.
//!
//! Split out of the former monolithic `sorted_set.rs` (phase2 modularization).
//! The value type (`SortedSetValue`) and its helpers (`OrderedFloat`,
//! `ScoredMember`, `ZAddOptions`) live in the parent module; this file holds
//! the store-level API, its stats and the `Aggregate` mode.
use super::{ScoredMember, SortedSetValue, ZAddOptions};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Statistics for sorted set store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortedSetStats {
    pub total_keys: usize,
    pub total_members: usize,
    pub avg_members_per_key: f64,
    pub memory_bytes: usize,
}

/// Aggregation method for set operations
#[derive(Debug, Clone, Copy)]
pub enum Aggregate {
    Sum,
    Min,
    Max,
}

/// Sorted Set store with 64-way sharding
pub struct SortedSetStore {
    shards: [Arc<RwLock<HashMap<String, SortedSetValue>>>; 64],
    /// Shared cross-datatype memory budget (audit M-018).
    mem: Option<crate::core::GlobalMemory>,
    mem_bytes: Arc<std::sync::atomic::AtomicI64>,
    /// Per-key broadcast channels used to wake `BZPOPMIN`/`BZPOPMAX` waiters when
    /// a member is added to a key (mirrors the list store's blocking-pop notify).
    notify_tx: Arc<RwLock<HashMap<String, tokio::sync::broadcast::Sender<()>>>>,
    /// Number of keys with a notify channel — lets `notify_waiters` skip the map
    /// entirely when no blocking pop has ever registered (the common case).
    notify_channels: std::sync::atomic::AtomicU64,
    /// Optional keyspace-notification publisher (Redis `notify-keyspace-events`).
    keyspace_notifier: Option<Arc<crate::core::KeyspaceNotifier>>,
}

impl SortedSetStore {
    /// Dump every sorted set (key -> Vec<(member, score)>) across all shards, for snapshotting.
    pub fn dump(&self) -> HashMap<String, Vec<(Vec<u8>, f64)>> {
        let mut out = HashMap::new();
        for shard in self.shards.iter() {
            let guard = shard.read();
            for (key, v) in guard.iter() {
                let members = v
                    .members_with_scores()
                    .into_iter()
                    .map(|sm| (sm.member, sm.score))
                    .collect();
                out.insert(key.clone(), members);
            }
        }
        out
    }

    /// Create a new sorted set store
    pub fn new() -> Self {
        Self {
            // Build the fixed-size shard array directly — no fallible Vec→array
            // conversion, so no panic on a length that is proven correct.
            shards: std::array::from_fn(|_| Arc::new(RwLock::new(HashMap::new()))),
            mem: None,
            mem_bytes: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            notify_tx: Arc::new(RwLock::new(HashMap::new())),
            notify_channels: std::sync::atomic::AtomicU64::new(0),
            keyspace_notifier: None,
        }
    }

    /// Wake any `BZPOPMIN`/`BZPOPMAX` waiters blocked on `key`.
    ///
    /// Fast path: skip the lock + lookup entirely while no blocking pop has ever
    /// registered a channel — the common case, so ZADD never touches the map.
    fn notify_waiters(&self, key: &str) {
        if self
            .notify_channels
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
        {
            return;
        }
        if let Some(tx) = self.notify_tx.read().get(key) {
            let _ = tx.send(());
        }
    }

    /// Get (or lazily create) the broadcast receiver blocked pops wait on.
    fn get_or_create_channel(&self, key: &str) -> tokio::sync::broadcast::Receiver<()> {
        let mut notify = self.notify_tx.write();
        notify
            .entry(key.to_string())
            .or_insert_with(|| {
                self.notify_channels
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                tokio::sync::broadcast::channel(100).0
            })
            .subscribe()
    }

    /// BZPOPMIN — pop the lowest-scored member from the first non-empty key,
    /// blocking up to `timeout_secs` (0/None = wait indefinitely) until a member
    /// is available. Returns `(key, member, score)`.
    pub async fn bzpopmin(
        &self,
        keys: Vec<String>,
        timeout_secs: Option<u64>,
    ) -> Result<(String, Vec<u8>, f64), crate::core::SynapError> {
        self.bzpop(keys, timeout_secs, true).await
    }

    /// BZPOPMAX — like [`bzpopmin`](Self::bzpopmin) but pops the highest score.
    pub async fn bzpopmax(
        &self,
        keys: Vec<String>,
        timeout_secs: Option<u64>,
    ) -> Result<(String, Vec<u8>, f64), crate::core::SynapError> {
        self.bzpop(keys, timeout_secs, false).await
    }

    async fn bzpop(
        &self,
        keys: Vec<String>,
        timeout_secs: Option<u64>,
        min: bool,
    ) -> Result<(String, Vec<u8>, f64), crate::core::SynapError> {
        // Immediate attempt across all keys, in order.
        let pop = |key: &str| -> Option<ScoredMember> {
            let popped = if min {
                self.zpopmin(key, 1)
            } else {
                self.zpopmax(key, 1)
            };
            popped.into_iter().next()
        };
        for key in &keys {
            if let Some(sm) = pop(key) {
                return Ok((key.clone(), sm.member, sm.score));
            }
        }

        let wait = async {
            let mut receivers: Vec<_> =
                keys.iter().map(|k| self.get_or_create_channel(k)).collect();
            loop {
                for rx in &mut receivers {
                    let _ = rx.recv().await;
                }
                for key in &keys {
                    if let Some(sm) = pop(key) {
                        return (key.clone(), sm.member, sm.score);
                    }
                }
            }
        };

        match timeout_secs {
            Some(secs) if secs > 0 => {
                match tokio::time::timeout(std::time::Duration::from_secs(secs), wait).await {
                    Ok(v) => Ok(v),
                    Err(_) => Err(crate::core::SynapError::Timeout),
                }
            }
            _ => Ok(wait.await),
        }
    }

    /// Attach the shared cross-datatype memory budget (audit M-018).
    pub fn with_global_memory(mut self, mem: crate::core::GlobalMemory) -> Self {
        mem.register(Arc::clone(&self.mem_bytes));
        self.mem = Some(mem);
        self
    }

    /// Attach a keyspace-notification publisher so sorted-set mutations publish
    /// `z`-class events. A no-op when `notifier` is `None`.
    pub fn with_keyspace_notifier(
        mut self,
        notifier: Option<Arc<crate::core::KeyspaceNotifier>>,
    ) -> Self {
        self.keyspace_notifier = notifier;
        self
    }

    /// Publish a sorted-set keyspace notification for `key` if a notifier is attached.
    #[inline]
    fn notify_keyspace(&self, event: &str, key: &str) {
        if let Some(ref n) = self.keyspace_notifier {
            n.notify(crate::core::EventClass::SortedSet, event, key);
        }
    }

    /// Total payload bytes currently held (keys + members + 8B/score across shards).
    pub fn memory_bytes(&self) -> usize {
        let mut total = 0usize;
        for shard in self.shards.iter() {
            for (key, v) in shard.read().iter() {
                total += key.len();
                for sm in v.members_with_scores() {
                    total += sm.member.len() + std::mem::size_of::<f64>();
                }
            }
        }
        total
    }

    /// Recompute this store's accounted memory into its registered counter.
    ///
    /// Note: `zadd` returns `(usize, usize)` rather than `Result`, so the
    /// sorted-set store contributes to the shared `maxmemory` total (and is thus
    /// subject to eviction/refusal on the other write paths) but does not itself
    /// reject a `zadd` when over budget.
    pub fn refresh_memory(&self) {
        if self.mem.is_some() {
            self.mem_bytes.store(
                self.memory_bytes() as i64,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
    }

    /// Get shard index for key
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() % 64) as usize
    }

    /// Get or create sorted set
    fn get_or_create(&self, key: &str) -> Arc<RwLock<HashMap<String, SortedSetValue>>> {
        let idx = self.shard_index(key);
        self.shards[idx].clone()
    }

    /// Add member with score to sorted set
    pub fn zadd(
        &self,
        key: &str,
        member: Vec<u8>,
        score: f64,
        opts: &ZAddOptions,
    ) -> (usize, usize) {
        let result = {
            let shard = self.get_or_create(key);
            let mut map = shard.write();
            let zset = map.entry(key.to_string()).or_default();
            zset.zadd(member, score, opts)
        };
        // A member is now available — wake any blocked BZPOPMIN/BZPOPMAX waiter.
        self.notify_waiters(key);
        self.notify_keyspace("zadd", key);
        result
    }

    /// Remove members from sorted set
    pub fn zrem(&self, key: &str, members: &[Vec<u8>]) -> usize {
        let shard = self.get_or_create(key);
        let removed = {
            let mut map = shard.write();
            if let Some(zset) = map.get_mut(key) {
                zset.zrem(members)
            } else {
                0
            }
        };
        if removed > 0 {
            self.notify_keyspace("zrem", key);
        }
        removed
    }

    /// Get score of member
    pub fn zscore(&self, key: &str, member: &[u8]) -> Option<f64> {
        let shard = self.get_or_create(key);
        let map = shard.read();
        map.get(key).and_then(|zset| zset.zscore(member))
    }

    /// Get cardinality
    pub fn zcard(&self, key: &str) -> usize {
        let shard = self.get_or_create(key);
        let map = shard.read();
        map.get(key).map(|zset| zset.zcard()).unwrap_or(0)
    }

    /// Increment score
    pub fn zincrby(&self, key: &str, member: Vec<u8>, increment: f64) -> f64 {
        let shard = self.get_or_create(key);
        let mut map = shard.write();
        let zset = map.entry(key.to_string()).or_default();
        zset.zincrby(member, increment)
    }

    /// Get range by rank
    pub fn zrange(&self, key: &str, start: i64, stop: i64, with_scores: bool) -> Vec<ScoredMember> {
        let shard = self.get_or_create(key);
        let map = shard.read();
        map.get(key)
            .map(|zset| zset.zrange(start, stop, with_scores))
            .unwrap_or_default()
    }

    /// Get reverse range by rank
    pub fn zrevrange(
        &self,
        key: &str,
        start: i64,
        stop: i64,
        with_scores: bool,
    ) -> Vec<ScoredMember> {
        let shard = self.get_or_create(key);
        let map = shard.read();
        map.get(key)
            .map(|zset| zset.zrevrange(start, stop, with_scores))
            .unwrap_or_default()
    }

    /// Get rank of member
    pub fn zrank(&self, key: &str, member: &[u8]) -> Option<usize> {
        let shard = self.get_or_create(key);
        let map = shard.read();
        map.get(key).and_then(|zset| zset.zrank(member))
    }

    /// Get reverse rank of member
    pub fn zrevrank(&self, key: &str, member: &[u8]) -> Option<usize> {
        let shard = self.get_or_create(key);
        let map = shard.read();
        map.get(key).and_then(|zset| zset.zrevrank(member))
    }

    /// Count members in score range
    pub fn zcount(&self, key: &str, min: f64, max: f64) -> usize {
        let shard = self.get_or_create(key);
        let map = shard.read();
        map.get(key).map(|zset| zset.zcount(min, max)).unwrap_or(0)
    }

    /// Pop minimum scored members
    pub fn zpopmin(&self, key: &str, count: usize) -> Vec<ScoredMember> {
        let shard = self.get_or_create(key);
        let mut map = shard.write();
        map.get_mut(key)
            .map(|zset| zset.zpopmin(count))
            .unwrap_or_default()
    }

    /// Pop maximum scored members
    pub fn zpopmax(&self, key: &str, count: usize) -> Vec<ScoredMember> {
        let shard = self.get_or_create(key);
        let mut map = shard.write();
        map.get_mut(key)
            .map(|zset| zset.zpopmax(count))
            .unwrap_or_default()
    }

    /// Get statistics
    pub fn stats(&self) -> SortedSetStats {
        let mut total_keys = 0;
        let mut total_members = 0;

        for shard in &self.shards {
            let map = shard.read();
            total_keys += map.len();
            for zset in map.values() {
                total_members += zset.zcard();
            }
        }

        let avg_members = if total_keys > 0 {
            total_members as f64 / total_keys as f64
        } else {
            0.0
        };

        SortedSetStats {
            total_keys,
            total_members,
            avg_members_per_key: avg_members,
            memory_bytes: total_keys * 128 + total_members * 48, // Rough estimate
        }
    }

    /// Delete a sorted set
    pub fn delete(&self, key: &str) -> bool {
        let shard = self.get_or_create(key);
        let mut map = shard.write();
        map.remove(key).is_some()
    }

    /// Compute intersection of multiple sorted sets and store in destination
    /// Returns count of members in result
    pub fn zinterstore(
        &self,
        destination: &str,
        keys: &[&str],
        weights: Option<&[f64]>,
        aggregate: Aggregate,
    ) -> usize {
        if keys.is_empty() {
            return 0;
        }

        let default_weights = vec![1.0; keys.len()];
        let weights = weights.unwrap_or(&default_weights);

        // Read all source sets
        let mut sets: Vec<HashMap<Vec<u8>, f64>> = Vec::new();
        for (i, key) in keys.iter().enumerate() {
            let shard = self.get_or_create(key);
            let map = shard.read();
            if let Some(zset) = map.get(*key) {
                let mut weighted_set = HashMap::new();
                for (member, score) in &zset.scores {
                    weighted_set.insert(member.clone(), score.get() * weights[i]);
                }
                sets.push(weighted_set);
            } else {
                // If any set doesn't exist, intersection is empty
                return 0;
            }
        }

        // Compute intersection
        let mut result = HashMap::new();
        if let Some(first_set) = sets.first() {
            for (member, first_score) in first_set {
                // Check if member exists in all sets
                let mut all_scores = vec![*first_score];
                let mut exists_in_all = true;

                for set in sets.iter().skip(1) {
                    if let Some(score) = set.get(member) {
                        all_scores.push(*score);
                    } else {
                        exists_in_all = false;
                        break;
                    }
                }

                if exists_in_all {
                    let aggregated = match aggregate {
                        Aggregate::Sum => all_scores.iter().sum(),
                        Aggregate::Min => all_scores.iter().cloned().fold(f64::INFINITY, f64::min),
                        Aggregate::Max => {
                            all_scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                        }
                    };
                    result.insert(member.clone(), aggregated);
                }
            }
        }

        // Store result
        let count = result.len();
        let dest_shard = self.get_or_create(destination);
        let mut dest_map = dest_shard.write();

        let mut dest_zset = SortedSetValue::new();
        let opts = ZAddOptions::default();
        for (member, score) in result {
            dest_zset.zadd(member, score, &opts);
        }

        dest_map.insert(destination.to_string(), dest_zset);
        count
    }

    /// Compute union of multiple sorted sets and store in destination
    /// Returns count of members in result
    pub fn zunionstore(
        &self,
        destination: &str,
        keys: &[&str],
        weights: Option<&[f64]>,
        aggregate: Aggregate,
    ) -> usize {
        if keys.is_empty() {
            return 0;
        }

        let default_weights = vec![1.0; keys.len()];
        let weights = weights.unwrap_or(&default_weights);

        // Read all source sets and collect all members
        let mut all_members: HashMap<Vec<u8>, Vec<f64>> = HashMap::new();

        for (i, key) in keys.iter().enumerate() {
            let shard = self.get_or_create(key);
            let map = shard.read();
            if let Some(zset) = map.get(*key) {
                for (member, score) in &zset.scores {
                    all_members
                        .entry(member.clone())
                        .or_default()
                        .push(score.get() * weights[i]);
                }
            }
        }

        // Aggregate scores
        let mut result = HashMap::new();
        for (member, scores) in all_members {
            let aggregated = match aggregate {
                Aggregate::Sum => scores.iter().sum(),
                Aggregate::Min => scores.iter().cloned().fold(f64::INFINITY, f64::min),
                Aggregate::Max => scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            };
            result.insert(member, aggregated);
        }

        // Store result
        let count = result.len();
        let dest_shard = self.get_or_create(destination);
        let mut dest_map = dest_shard.write();

        let mut dest_zset = SortedSetValue::new();
        let opts = ZAddOptions::default();
        for (member, score) in result {
            dest_zset.zadd(member, score, &opts);
        }

        dest_map.insert(destination.to_string(), dest_zset);
        count
    }

    /// Compute difference of first set minus other sets and store in destination
    /// Returns count of members in result
    pub fn zdiffstore(&self, destination: &str, keys: &[&str]) -> usize {
        if keys.is_empty() {
            return 0;
        }

        // Read first set
        let first_key = keys[0];
        let first_shard = self.get_or_create(first_key);
        let first_map = first_shard.read();

        let first_set = match first_map.get(first_key) {
            Some(zset) => zset.scores.clone(),
            None => return 0, // First set doesn't exist, result is empty
        };
        drop(first_map);

        // Read other sets to subtract
        let mut subtract_members = std::collections::HashSet::new();
        for key in keys.iter().skip(1) {
            let shard = self.get_or_create(key);
            let map = shard.read();
            if let Some(zset) = map.get(*key) {
                for member in zset.scores.keys() {
                    subtract_members.insert(member.clone());
                }
            }
        }

        // Compute difference
        let mut result = HashMap::new();
        for (member, score) in first_set {
            if !subtract_members.contains(&member) {
                result.insert(member, score.get());
            }
        }

        // Store result
        let count = result.len();
        let dest_shard = self.get_or_create(destination);
        let mut dest_map = dest_shard.write();

        let mut dest_zset = SortedSetValue::new();
        let opts = ZAddOptions::default();
        for (member, score) in result {
            dest_zset.zadd(member, score, &opts);
        }

        dest_map.insert(destination.to_string(), dest_zset);
        count
    }

    /// Get range by score (wrapper for store)
    pub fn zrangebyscore(
        &self,
        key: &str,
        min: f64,
        max: f64,
        with_scores: bool,
    ) -> Vec<ScoredMember> {
        let shard = self.get_or_create(key);
        let map = shard.read();
        map.get(key)
            .map(|zset| zset.zrangebyscore(min, max, with_scores))
            .unwrap_or_default()
    }

    /// Remove members by rank range
    pub fn zremrangebyrank(&self, key: &str, start: i64, stop: i64) -> usize {
        let shard = self.get_or_create(key);
        let mut map = shard.write();
        map.get_mut(key)
            .map(|zset| zset.zremrangebyrank(start, stop))
            .unwrap_or(0)
    }

    /// Remove members by score range
    pub fn zremrangebyscore(&self, key: &str, min: f64, max: f64) -> usize {
        let shard = self.get_or_create(key);
        let mut map = shard.write();
        map.get_mut(key)
            .map(|zset| zset.zremrangebyscore(min, max))
            .unwrap_or(0)
    }

    /// Get multiple scores
    pub fn zmscore(&self, key: &str, members: &[Vec<u8>]) -> Vec<Option<f64>> {
        let shard = self.get_or_create(key);
        let map = shard.read();
        map.get(key)
            .map(|zset| zset.zmscore(members))
            .unwrap_or_else(|| vec![None; members.len()])
    }

    /// ZSCAN - cursor-based incremental scan of a sorted set's members.
    ///
    /// `cursor` is an offset into a member-sorted snapshot; returns the next
    /// cursor (0 when complete) and the matched `(member, score)` pairs within
    /// the window. `pattern` is an optional glob over members; `count` bounds the
    /// window size (min 1).
    pub fn zscan(
        &self,
        key: &str,
        cursor: u64,
        pattern: Option<&str>,
        count: usize,
    ) -> (u64, Vec<(Vec<u8>, f64)>) {
        let mut members: Vec<(Vec<u8>, f64)> = self
            .zrange(key, 0, -1, true)
            .into_iter()
            .map(|sm| (sm.member, sm.score))
            .collect();
        members.sort_by(|a, b| a.0.cmp(&b.0));
        let total = members.len();
        let start = (cursor as usize).min(total);
        let end = start.saturating_add(count.max(1)).min(total);
        let items = members[start..end]
            .iter()
            .filter(|(m, _)| {
                pattern.is_none_or(|p| crate::core::glob_match(p, &String::from_utf8_lossy(m)))
            })
            .cloned()
            .collect();
        let next = if end < total { end as u64 } else { 0 };
        (next, items)
    }
}

impl Default for SortedSetStore {
    fn default() -> Self {
        Self::new()
    }
}
