//! Sorted Set data structure implementation
//!
//! Provides Redis-compatible sorted sets with dual data structure:
//! - HashMap for O(1) member-to-score lookups
//! - BTreeMap for O(log n) range queries and ranking
//!
//! Use cases: leaderboards, priority queues, time-series, rate limiting

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Wrapper for f64 that provides total ordering
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OrderedFloat(pub f64);

impl OrderedFloat {
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    pub fn get(&self) -> f64 {
        self.0
    }
}

impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        // Handle NaN by treating it as less than everything
        match (self.0.is_nan(), other.0.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal),
        }
    }
}

impl Hash for OrderedFloat {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

/// A member-score pair in a sorted set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredMember {
    pub member: Vec<u8>,
    pub score: f64,
}

/// Options for ZADD command
#[derive(Debug, Clone, Default)]
pub struct ZAddOptions {
    /// Only add new elements (NX)
    pub nx: bool,
    /// Only update existing elements (XX)
    pub xx: bool,
    /// Only update if new score > old score (GT)
    pub gt: bool,
    /// Only update if new score < old score (LT)
    pub lt: bool,
    /// Return count of changed elements instead of added (CH)
    pub ch: bool,
    /// Increment score instead of replace (INCR)
    pub incr: bool,
}

/// A sorted set with dual data structure
#[derive(Debug)]
pub struct SortedSetValue {
    /// Member -> Score mapping (O(1) lookup)
    scores: HashMap<Vec<u8>, OrderedFloat>,
    /// (Score, Member) -> () sorted index (O(log n) range queries)
    sorted: BTreeMap<(OrderedFloat, Vec<u8>), ()>,
    /// TTL expiration timestamp (Unix seconds), None = no expiration
    expires_at: Option<u32>,
    /// Creation timestamp
    #[allow(dead_code)]
    created_at: u32,
}

impl SortedSetValue {
    /// Create a new empty sorted set
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
            sorted: BTreeMap::new(),
            expires_at: None,
            created_at: Self::current_timestamp(),
        }
    }

    /// Create with TTL in seconds
    pub fn with_ttl(ttl_secs: u32) -> Self {
        let now = Self::current_timestamp();
        Self {
            scores: HashMap::new(),
            sorted: BTreeMap::new(),
            expires_at: Some(now + ttl_secs),
            created_at: now,
        }
    }

    /// Get current Unix timestamp in seconds
    fn current_timestamp() -> u32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32
    }

    /// Check if expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Self::current_timestamp() >= expires_at
        } else {
            false
        }
    }

    /// Add or update a member with score
    /// Returns (added_count, changed_count)
    pub fn zadd(&mut self, member: Vec<u8>, score: f64, opts: &ZAddOptions) -> (usize, usize) {
        let ordered_score = OrderedFloat::new(score);
        let exists = self.scores.contains_key(&member);

        // Handle NX/XX options
        if opts.nx && exists {
            return (0, 0);
        }
        if opts.xx && !exists {
            return (0, 0);
        }

        let mut added = 0;
        let mut changed = 0;

        if let Some(&old_score) = self.scores.get(&member) {
            // Member exists - check GT/LT options
            if opts.gt && score <= old_score.get() {
                return (0, 0);
            }
            if opts.lt && score >= old_score.get() {
                return (0, 0);
            }

            // Remove old entry from sorted index
            self.sorted.remove(&(old_score, member.clone()));

            // Handle INCR option
            let new_score = if opts.incr {
                old_score.get() + score
            } else {
                score
            };

            // Update score
            let new_ordered_score = OrderedFloat::new(new_score);
            self.scores.insert(member.clone(), new_ordered_score);
            self.sorted.insert((new_ordered_score, member), ());

            if old_score != new_ordered_score {
                changed = 1;
            }
        } else {
            // New member
            self.scores.insert(member.clone(), ordered_score);
            self.sorted.insert((ordered_score, member), ());
            added = 1;
            changed = 1;
        }

        if opts.ch { (0, changed) } else { (added, 0) }
    }

    /// Remove members
    /// Returns count of removed members
    pub fn zrem(&mut self, members: &[Vec<u8>]) -> usize {
        let mut removed = 0;
        for member in members {
            if let Some(score) = self.scores.remove(member) {
                self.sorted.remove(&(score, member.clone()));
                removed += 1;
            }
        }
        removed
    }

    /// Get score of a member
    pub fn zscore(&self, member: &[u8]) -> Option<f64> {
        self.scores.get(member).map(|s| s.get())
    }

    /// Get cardinality (count of members)
    pub fn zcard(&self) -> usize {
        self.scores.len()
    }

    /// Increment score of member
    /// Returns new score
    pub fn zincrby(&mut self, member: Vec<u8>, increment: f64) -> f64 {
        let old_score = self.scores.get(&member).map(|s| s.get()).unwrap_or(0.0);
        let new_score = old_score + increment;

        // Remove old entry if exists
        if let Some(old) = self.scores.get(&member) {
            self.sorted.remove(&(*old, member.clone()));
        }

        // Add new entry
        let ordered = OrderedFloat::new(new_score);
        self.scores.insert(member.clone(), ordered);
        self.sorted.insert((ordered, member), ());

        new_score
    }

    /// Get range by rank (0-based index)
    /// Returns members with optional scores
    pub fn zrange(&self, start: i64, stop: i64, _with_scores: bool) -> Vec<ScoredMember> {
        let len = self.scores.len() as i64;
        if len == 0 {
            return Vec::new();
        }

        // Convert negative indices
        let start = if start < 0 { len + start } else { start };
        let stop = if stop < 0 { len + stop } else { stop };

        // Clamp to valid range
        let start = start.max(0) as usize;
        let stop = (stop + 1).min(len) as usize;

        if start >= stop {
            return Vec::new();
        }

        self.sorted
            .iter()
            .skip(start)
            .take(stop - start)
            .map(|((score, member), _)| ScoredMember {
                member: member.clone(),
                score: score.get(),
            })
            .collect()
    }

    /// Get reverse range by rank
    pub fn zrevrange(&self, start: i64, stop: i64, _with_scores: bool) -> Vec<ScoredMember> {
        let len = self.scores.len() as i64;
        if len == 0 {
            return Vec::new();
        }

        // Convert negative indices
        let start = if start < 0 { len + start } else { start };
        let stop = if stop < 0 { len + stop } else { stop };

        // Clamp to valid range
        let start = start.max(0) as usize;
        let stop = (stop + 1).min(len) as usize;

        if start >= stop {
            return Vec::new();
        }

        self.sorted
            .iter()
            .rev()
            .skip(start)
            .take(stop - start)
            .map(|((score, member), _)| ScoredMember {
                member: member.clone(),
                score: score.get(),
            })
            .collect()
    }

    /// Get rank of member (0-based)
    pub fn zrank(&self, member: &[u8]) -> Option<usize> {
        let score = self.scores.get(member)?;
        let rank = self.sorted.range(..(*score, member.to_vec())).count();
        Some(rank)
    }

    /// Get reverse rank of member (0-based from highest)
    pub fn zrevrank(&self, member: &[u8]) -> Option<usize> {
        let rank = self.zrank(member)?;
        Some(self.scores.len() - rank - 1)
    }

    /// Count members with scores in range
    pub fn zcount(&self, min: f64, max: f64) -> usize {
        let min_key = OrderedFloat::new(min);
        let max_key = OrderedFloat::new(max);

        self.sorted
            .range((min_key, Vec::new())..=(max_key, vec![u8::MAX; 256]))
            .count()
    }

    /// Pop minimum scored members
    pub fn zpopmin(&mut self, count: usize) -> Vec<ScoredMember> {
        let mut result = Vec::new();

        for _ in 0..count {
            if let Some(((score, member), _)) = self.sorted.iter().next() {
                let score_val = score.get();
                let member_clone = member.clone();

                self.sorted.remove(&(*score, member.clone()));
                self.scores.remove(&member_clone);

                result.push(ScoredMember {
                    member: member_clone,
                    score: score_val,
                });
            } else {
                break;
            }
        }

        result
    }

    /// Pop maximum scored members
    pub fn zpopmax(&mut self, count: usize) -> Vec<ScoredMember> {
        let mut result = Vec::new();

        for _ in 0..count {
            if let Some(((score, member), _)) = self.sorted.iter().next_back() {
                let score_val = score.get();
                let member_clone = member.clone();

                self.sorted.remove(&(*score, member.clone()));
                self.scores.remove(&member_clone);

                result.push(ScoredMember {
                    member: member_clone,
                    score: score_val,
                });
            } else {
                break;
            }
        }

        result
    }

    /// Set TTL in seconds
    pub fn set_ttl(&mut self, ttl_secs: u32) {
        let now = Self::current_timestamp();
        self.expires_at = Some(now + ttl_secs);
    }

    /// Remove TTL
    pub fn persist(&mut self) {
        self.expires_at = None;
    }

    /// Get TTL in seconds
    pub fn ttl(&self) -> Option<i64> {
        self.expires_at.map(|expires_at| {
            let now = Self::current_timestamp();
            (expires_at as i64) - (now as i64)
        })
    }

    /// Get all members with scores
    pub fn members_with_scores(&self) -> Vec<ScoredMember> {
        self.sorted
            .iter()
            .map(|((score, member), _)| ScoredMember {
                member: member.clone(),
                score: score.get(),
            })
            .collect()
    }

    /// Remove members by rank range
    pub fn zremrangebyrank(&mut self, start: i64, stop: i64) -> usize {
        let range = self.zrange(start, stop, false);
        let members: Vec<Vec<u8>> = range.into_iter().map(|m| m.member).collect();
        self.zrem(&members)
    }

    /// Remove members by score range
    pub fn zremrangebyscore(&mut self, min: f64, max: f64) -> usize {
        let min_key = OrderedFloat::new(min);
        let max_key = OrderedFloat::new(max);

        let to_remove: Vec<Vec<u8>> = self
            .sorted
            .range((min_key, Vec::new())..=(max_key, vec![u8::MAX; 256]))
            .map(|((_, member), _)| member.clone())
            .collect();

        self.zrem(&to_remove)
    }

    /// Get range by score
    pub fn zrangebyscore(&self, min: f64, max: f64, _with_scores: bool) -> Vec<ScoredMember> {
        let min_key = OrderedFloat::new(min);
        let max_key = OrderedFloat::new(max);

        self.sorted
            .range((min_key, Vec::new())..=(max_key, vec![u8::MAX; 256]))
            .map(|((score, member), _)| ScoredMember {
                member: member.clone(),
                score: score.get(),
            })
            .collect()
    }

    /// Get multiple scores
    pub fn zmscore(&self, members: &[Vec<u8>]) -> Vec<Option<f64>> {
        members.iter().map(|member| self.zscore(member)).collect()
    }
}

impl Default for SortedSetValue {
    fn default() -> Self {
        Self::new()
    }
}

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
            keyspace_notifier: None,
        }
    }

    /// Wake any `BZPOPMIN`/`BZPOPMAX` waiters blocked on `key`.
    fn notify_waiters(&self, key: &str) {
        if let Some(tx) = self.notify_tx.read().get(key) {
            let _ = tx.send(());
        }
    }

    /// Get (or lazily create) the broadcast receiver blocked pops wait on.
    fn get_or_create_channel(&self, key: &str) -> tokio::sync::broadcast::Receiver<()> {
        let mut notify = self.notify_tx.write();
        notify
            .entry(key.to_string())
            .or_insert_with(|| tokio::sync::broadcast::channel(100).0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordered_float_ordering() {
        assert!(OrderedFloat::new(1.0) < OrderedFloat::new(2.0));
        assert!(OrderedFloat::new(2.0) > OrderedFloat::new(1.0));
        assert_eq!(OrderedFloat::new(1.0), OrderedFloat::new(1.0));

        // NaN handling
        let nan = OrderedFloat::new(f64::NAN);
        let num = OrderedFloat::new(1.0);
        assert!(nan < num);
    }

    #[test]
    fn test_zadd_basic() {
        let mut zset = SortedSetValue::new();
        let opts = ZAddOptions::default();

        let (added, _) = zset.zadd(b"member1".to_vec(), 1.0, &opts);
        assert_eq!(added, 1);
        assert_eq!(zset.zcard(), 1);
        assert_eq!(zset.zscore(b"member1"), Some(1.0));
    }

    #[test]
    fn test_zadd_update() {
        let mut zset = SortedSetValue::new();
        let opts = ZAddOptions::default();

        zset.zadd(b"member1".to_vec(), 1.0, &opts);
        let (added, _) = zset.zadd(b"member1".to_vec(), 2.0, &opts);

        assert_eq!(added, 0); // Not added, updated
        assert_eq!(zset.zcard(), 1);
        assert_eq!(zset.zscore(b"member1"), Some(2.0));
    }

    #[test]
    fn test_zadd_nx() {
        let mut zset = SortedSetValue::new();
        let opts_default = ZAddOptions::default();
        let opts_nx = ZAddOptions {
            nx: true,
            ..Default::default()
        };

        zset.zadd(b"member1".to_vec(), 1.0, &opts_default);
        let (added, _) = zset.zadd(b"member1".to_vec(), 2.0, &opts_nx);

        assert_eq!(added, 0); // NX prevents update
        assert_eq!(zset.zscore(b"member1"), Some(1.0)); // Score unchanged
    }

    #[test]
    fn test_zrem() {
        let mut zset = SortedSetValue::new();
        let opts = ZAddOptions::default();

        zset.zadd(b"member1".to_vec(), 1.0, &opts);
        zset.zadd(b"member2".to_vec(), 2.0, &opts);

        let removed = zset.zrem(&[b"member1".to_vec()]);
        assert_eq!(removed, 1);
        assert_eq!(zset.zcard(), 1);
        assert_eq!(zset.zscore(b"member1"), None);
    }

    #[test]
    fn test_zincrby() {
        let mut zset = SortedSetValue::new();

        let score = zset.zincrby(b"member1".to_vec(), 1.0);
        assert_eq!(score, 1.0);

        let score = zset.zincrby(b"member1".to_vec(), 2.5);
        assert_eq!(score, 3.5);
    }

    #[test]
    fn test_zrange() {
        let mut zset = SortedSetValue::new();
        let opts = ZAddOptions::default();

        zset.zadd(b"a".to_vec(), 1.0, &opts);
        zset.zadd(b"b".to_vec(), 2.0, &opts);
        zset.zadd(b"c".to_vec(), 3.0, &opts);

        let range = zset.zrange(0, 1, true);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].member, b"a");
        assert_eq!(range[0].score, 1.0);
        assert_eq!(range[1].member, b"b");
        assert_eq!(range[1].score, 2.0);
    }

    #[test]
    fn test_zrank() {
        let mut zset = SortedSetValue::new();
        let opts = ZAddOptions::default();

        zset.zadd(b"a".to_vec(), 1.0, &opts);
        zset.zadd(b"b".to_vec(), 2.0, &opts);
        zset.zadd(b"c".to_vec(), 3.0, &opts);

        assert_eq!(zset.zrank(b"a"), Some(0));
        assert_eq!(zset.zrank(b"b"), Some(1));
        assert_eq!(zset.zrank(b"c"), Some(2));
    }

    #[test]
    fn test_zpopmin() {
        let mut zset = SortedSetValue::new();
        let opts = ZAddOptions::default();

        zset.zadd(b"a".to_vec(), 1.0, &opts);
        zset.zadd(b"b".to_vec(), 2.0, &opts);
        zset.zadd(b"c".to_vec(), 3.0, &opts);

        let popped = zset.zpopmin(2);
        assert_eq!(popped.len(), 2);
        assert_eq!(popped[0].member, b"a");
        assert_eq!(popped[1].member, b"b");
        assert_eq!(zset.zcard(), 1);
    }

    #[test]
    fn test_store_basic() {
        let store = SortedSetStore::new();
        let opts = ZAddOptions::default();

        store.zadd("zset1", b"member1".to_vec(), 1.0, &opts);
        assert_eq!(store.zcard("zset1"), 1);
        assert_eq!(store.zscore("zset1", b"member1"), Some(1.0));
    }

    fn seed(store: &SortedSetStore, key: &str, pairs: &[(&str, f64)]) {
        let opts = ZAddOptions::default();
        for (m, s) in pairs {
            store.zadd(key, m.as_bytes().to_vec(), *s, &opts);
        }
    }

    #[test]
    fn test_zadd_option_flags() {
        let store = SortedSetStore::new();
        let nx = ZAddOptions {
            nx: true,
            ..Default::default()
        };
        store.zadd("z", b"a".to_vec(), 1.0, &nx);
        // NX must not overwrite an existing member.
        store.zadd("z", b"a".to_vec(), 9.0, &nx);
        assert_eq!(store.zscore("z", b"a"), Some(1.0));

        // XX only updates existing members.
        let xx = ZAddOptions {
            xx: true,
            ..Default::default()
        };
        store.zadd("z", b"new".to_vec(), 5.0, &xx);
        assert_eq!(store.zscore("z", b"new"), None);
        store.zadd("z", b"a".to_vec(), 2.0, &xx);
        assert_eq!(store.zscore("z", b"a"), Some(2.0));

        // GT only raises, LT only lowers.
        let gt = ZAddOptions {
            gt: true,
            ..Default::default()
        };
        store.zadd("z", b"a".to_vec(), 1.0, &gt); // lower → ignored
        assert_eq!(store.zscore("z", b"a"), Some(2.0));
        store.zadd("z", b"a".to_vec(), 10.0, &gt); // higher → applied
        assert_eq!(store.zscore("z", b"a"), Some(10.0));
        let lt = ZAddOptions {
            lt: true,
            ..Default::default()
        };
        store.zadd("z", b"a".to_vec(), 3.0, &lt);
        assert_eq!(store.zscore("z", b"a"), Some(3.0));

        // INCR increments instead of replacing.
        let incr = ZAddOptions {
            incr: true,
            ..Default::default()
        };
        store.zadd("z", b"a".to_vec(), 2.0, &incr);
        assert_eq!(store.zscore("z", b"a"), Some(5.0));
    }

    #[test]
    fn test_range_and_rank_queries() {
        let store = SortedSetStore::new();
        seed(
            &store,
            "z",
            &[("a", 1.0), ("b", 2.0), ("c", 3.0), ("d", 4.0)],
        );

        let fwd = store.zrange("z", 0, -1, true);
        assert_eq!(
            fwd.iter().map(|m| m.member.clone()).collect::<Vec<_>>(),
            vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec(), b"d".to_vec()]
        );
        let rev = store.zrevrange("z", 0, 1, false);
        assert_eq!(rev[0].member, b"d".to_vec());
        assert_eq!(rev[1].member, b"c".to_vec());

        assert_eq!(store.zrank("z", b"c"), Some(2));
        assert_eq!(store.zrevrank("z", b"c"), Some(1));
        assert_eq!(store.zrank("z", b"missing"), None);

        assert_eq!(store.zcount("z", 2.0, 3.0), 2);
        let by_score = store.zrangebyscore("z", 2.0, 3.0, false);
        assert_eq!(by_score.len(), 2);

        assert_eq!(
            store.zmscore("z", &[b"a".to_vec(), b"x".to_vec()]),
            vec![Some(1.0), None]
        );
    }

    #[test]
    fn test_pop_and_remove_ranges() {
        let store = SortedSetStore::new();
        seed(
            &store,
            "z",
            &[("a", 1.0), ("b", 2.0), ("c", 3.0), ("d", 4.0)],
        );

        let min = store.zpopmin("z", 1);
        assert_eq!(min[0].member, b"a".to_vec());
        let max = store.zpopmax("z", 1);
        assert_eq!(max[0].member, b"d".to_vec());
        // b, c remain.
        assert_eq!(store.zcard("z"), 2);

        assert_eq!(store.zremrangebyrank("z", 0, 0), 1); // removes b
        assert_eq!(store.zscore("z", b"b"), None);

        seed(&store, "z2", &[("a", 1.0), ("b", 2.0), ("c", 3.0)]);
        assert_eq!(store.zremrangebyscore("z2", 2.0, 3.0), 2);
        assert_eq!(store.zcard("z2"), 1);
    }

    #[test]
    fn test_incrby_rem_delete_and_stats() {
        let store = SortedSetStore::new();
        seed(&store, "z", &[("a", 1.0), ("b", 2.0)]);
        assert_eq!(store.zincrby("z", b"a".to_vec(), 4.5), 5.5);
        assert_eq!(store.zrem("z", &[b"b".to_vec()]), 1);

        let stats = store.stats();
        assert_eq!(stats.total_keys, 1);
        assert_eq!(stats.total_members, 1);
        assert!(store.memory_bytes() > 0);
        store.refresh_memory();

        assert!(store.delete("z"));
        assert!(!store.delete("z"));
        assert_eq!(store.zcard("z"), 0);
    }

    #[test]
    fn test_store_set_operations() {
        let store = SortedSetStore::new();
        seed(&store, "z1", &[("a", 1.0), ("b", 2.0), ("c", 3.0)]);
        seed(&store, "z2", &[("b", 10.0), ("c", 20.0), ("d", 30.0)]);

        // INTERSTORE with SUM aggregate.
        let n = store.zinterstore("zi", &["z1", "z2"], None, Aggregate::Sum);
        assert_eq!(n, 2); // b, c
        assert_eq!(store.zscore("zi", b"b"), Some(12.0));

        // UNIONSTORE with weights and MAX aggregate.
        let n = store.zunionstore("zu", &["z1", "z2"], Some(&[1.0, 2.0]), Aggregate::Max);
        assert_eq!(n, 4);
        // b: max(2*1, 10*2) = 20
        assert_eq!(store.zscore("zu", b"b"), Some(20.0));

        // DIFFSTORE keeps members of z1 not in z2.
        let n = store.zdiffstore("zd", &["z1", "z2"]);
        assert_eq!(n, 1);
        assert_eq!(store.zscore("zd", b"a"), Some(1.0));
    }

    #[test]
    fn test_value_ttl_helpers() {
        let mut v = SortedSetValue::with_ttl(3600);
        assert!(!v.is_expired());
        assert!(v.ttl().is_some());
        v.persist();
        assert_eq!(v.ttl(), None);
        v.set_ttl(60);
        assert!(v.ttl().is_some());

        let expired = SortedSetValue::with_ttl(0);
        assert!(expired.is_expired());
    }

    #[tokio::test]
    async fn test_bzpopmin_max_immediate() {
        let store = SortedSetStore::new();
        seed(&store, "z", &[("a", 1.0), ("b", 2.0), ("c", 3.0)]);

        let (k, m, s) = store.bzpopmin(vec!["z".into()], Some(1)).await.unwrap();
        assert_eq!(k, "z");
        assert_eq!(m, b"a".to_vec());
        assert_eq!(s, 1.0);

        let (_, m, s) = store.bzpopmax(vec!["z".into()], Some(1)).await.unwrap();
        assert_eq!(m, b"c".to_vec());
        assert_eq!(s, 3.0);
    }

    #[tokio::test]
    async fn test_bzpopmin_first_nonempty_key() {
        let store = SortedSetStore::new();
        seed(&store, "z2", &[("x", 5.0)]);
        // z1 empty, z2 has a member → pops from z2.
        let (k, m, _) = store
            .bzpopmin(vec!["z1".into(), "z2".into()], Some(1))
            .await
            .unwrap();
        assert_eq!(k, "z2");
        assert_eq!(m, b"x".to_vec());
    }

    #[tokio::test]
    async fn test_bzpopmin_times_out_when_empty() {
        let store = SortedSetStore::new();
        let err = store.bzpopmin(vec!["missing".into()], Some(1)).await;
        assert!(matches!(err, Err(crate::core::SynapError::Timeout)));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_bzpopmin_wakes_on_zadd() {
        let store = Arc::new(SortedSetStore::new());
        let waiter = Arc::clone(&store);
        let handle =
            tokio::spawn(async move { waiter.bzpopmin(vec!["z".into()], Some(5)).await.unwrap() });

        // Let the waiter block, then push a member — it must wake and pop it.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        store.zadd("z", b"m".to_vec(), 7.0, &ZAddOptions::default());

        let (k, m, s) = handle.await.unwrap();
        assert_eq!(k, "z");
        assert_eq!(m, b"m".to_vec());
        assert_eq!(s, 7.0);
    }

    #[test]
    fn test_zscan_cursor_and_match() {
        let store = SortedSetStore::new();
        seed(
            &store,
            "z",
            &[("a", 1.0), ("b", 2.0), ("c", 3.0), ("d", 4.0)],
        );

        let mut seen = Vec::new();
        let mut cursor = 0u64;
        loop {
            let (next, items) = store.zscan("z", cursor, None, 2);
            seen.extend(items);
            if next == 0 {
                break;
            }
            cursor = next;
        }
        seen.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            seen,
            vec![
                (b"a".to_vec(), 1.0),
                (b"b".to_vec(), 2.0),
                (b"c".to_vec(), 3.0),
                (b"d".to_vec(), 4.0),
            ]
        );

        let (_c, items) = store.zscan("z", 0, Some("[ab]"), 100);
        assert_eq!(items.len(), 2);
        assert_eq!(store.zscan("missing", 0, None, 10), (0, vec![]));
    }
}
