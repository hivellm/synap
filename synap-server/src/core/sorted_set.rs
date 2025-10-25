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
            .unwrap()
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

/// Sorted Set store with 64-way sharding
pub struct SortedSetStore {
    shards: [Arc<RwLock<HashMap<String, SortedSetValue>>>; 64],
}

impl SortedSetStore {
    /// Create a new sorted set store
    pub fn new() -> Self {
        let shards: Vec<_> = (0..64)
            .map(|_| Arc::new(RwLock::new(HashMap::new())))
            .collect();

        Self {
            shards: shards.try_into().unwrap(),
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
        let shard = self.get_or_create(key);
        let mut map = shard.write();
        let zset = map.entry(key.to_string()).or_default();
        zset.zadd(member, score, opts)
    }

    /// Remove members from sorted set
    pub fn zrem(&self, key: &str, members: &[Vec<u8>]) -> usize {
        let shard = self.get_or_create(key);
        let mut map = shard.write();
        if let Some(zset) = map.get_mut(key) {
            zset.zrem(members)
        } else {
            0
        }
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
}
