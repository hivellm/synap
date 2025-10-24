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
use parking_lot::RwLock;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const SHARD_COUNT: usize = 64;

/// Set value stored in a single key
/// Contains unique members
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetValue {
    /// Unique members
    pub members: HashSet<Vec<u8>>,
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
            members: HashSet::new(),
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
        self.members.len()
    }

    /// Check if set is empty
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Add member(s), returns number of members added
    pub fn add(&mut self, members: Vec<Vec<u8>>) -> usize {
        self.updated_at = Self::current_timestamp();
        let mut added = 0;
        for member in members {
            if self.members.insert(member) {
                added += 1;
            }
        }
        added
    }

    /// Remove member(s), returns number of members removed
    pub fn remove(&mut self, members: &[Vec<u8>]) -> usize {
        self.updated_at = Self::current_timestamp();
        members.iter().filter(|m| self.members.remove(*m)).count()
    }

    /// Check if member exists
    pub fn is_member(&self, member: &[u8]) -> bool {
        self.members.contains(member)
    }

    /// Get all members
    pub fn members(&self) -> Vec<Vec<u8>> {
        self.members.iter().cloned().collect()
    }

    /// Pop random member
    pub fn pop(&mut self, count: usize) -> Vec<Vec<u8>> {
        self.updated_at = Self::current_timestamp();
        let mut result = Vec::new();
        let members: Vec<_> = self.members.iter().cloned().collect();

        for member in members.iter().take(count) {
            if self.members.remove(member) {
                result.push(member.clone());
            }
        }

        result
    }

    /// Get random member(s) without removing
    pub fn random_members(&self, count: usize) -> Vec<Vec<u8>> {
        let mut members: Vec<_> = self.members.iter().cloned().collect();
        if count >= members.len() {
            members
        } else {
            let mut rng = rand::thread_rng();
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

/// Sharded set store with 64-way concurrency
pub struct SetStore {
    shards: Vec<Arc<RwLock<HashMap<String, SetValue>>>>,
    stats: Arc<RwLock<SetStats>>,
}

impl Default for SetStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SetStore {
    /// Create new set store
    pub fn new() -> Self {
        let mut shards = Vec::with_capacity(SHARD_COUNT);
        for _ in 0..SHARD_COUNT {
            shards.push(Arc::new(RwLock::new(HashMap::new())));
        }

        Self {
            shards,
            stats: Arc::new(RwLock::new(SetStats::default())),
        }
    }

    /// Get shard index for key
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % SHARD_COUNT
    }

    /// Get shard for key
    fn shard(&self, key: &str) -> &Arc<RwLock<HashMap<String, SetValue>>> {
        &self.shards[self.shard_index(key)]
    }

    /// SADD - Add member(s) to set
    pub fn sadd(&self, key: &str, members: Vec<Vec<u8>>) -> Result<usize> {
        let shard = self.shard(key);
        let mut map = shard.write();

        // Check expiration first
        if let Some(existing_set) = map.get(key) {
            if existing_set.is_expired() {
                map.remove(key);
            }
        }

        let set = map
            .entry(key.to_string())
            .or_insert_with(|| SetValue::new(None));

        let added = set.add(members);
        self.stats.write().sadd_count += 1;

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
        self.stats.write().srem_count += 1;

        // Remove empty sets
        if set.is_empty() {
            map.remove(key);
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

        self.stats.write().sismember_count += 1;
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

        self.stats.write().smembers_count += 1;
        Ok(set.members())
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

        self.stats.write().scard_count += 1;
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
        self.stats.write().spop_count += 1;

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
        self.stats.write().srandmember_count += 1;
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

            if !source_set.members.remove(&member) {
                return Ok(false); // Member not in source
            }

            // Remove empty source
            if source_set.is_empty() {
                map.remove(source);
            }

            // Add to destination
            if let Some(existing_set) = map.get(destination) {
                if existing_set.is_expired() {
                    map.remove(destination);
                }
            }

            let dest_set = map
                .entry(destination.to_string())
                .or_insert_with(|| SetValue::new(None));

            dest_set.members.insert(member);
            dest_set.updated_at = SetValue::current_timestamp();

            self.stats.write().smove_count += 1;
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

        if !source_set.members.remove(&member) {
            return Ok(false);
        }

        // Remove empty source
        if source_set.is_empty() {
            source_map.remove(source);
        }

        // Add to destination
        if let Some(existing_set) = dest_map.get(destination) {
            if existing_set.is_expired() {
                dest_map.remove(destination);
            }
        }

        let dest_set = dest_map
            .entry(destination.to_string())
            .or_insert_with(|| SetValue::new(None));

        dest_set.members.insert(member);
        dest_set.updated_at = SetValue::current_timestamp();

        self.stats.write().smove_count += 1;
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
                    .map(|s| s.members.clone())
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

        self.stats.write().sinter_count += 1;
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
            if let Some(set) = map.get(key) {
                if !set.is_expired() {
                    result.extend(set.members.iter().cloned());
                }
            }
        }

        self.stats.write().sunion_count += 1;
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
            .map(|s| s.members.clone())
            .unwrap_or_default();

        // Remove members from other sets
        for key in keys.iter().skip(1) {
            let shard = self.shard(key);
            let map = shard.read();
            if let Some(set) = map.get(key) {
                if !set.is_expired() {
                    result.retain(|member| !set.members.contains(member));
                }
            }
        }

        self.stats.write().sdiff_count += 1;
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
        set.members = members.into_iter().collect();
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
        set.members = members.into_iter().collect();
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
        set.members = members.into_iter().collect();
        map.insert(destination.to_string(), set);

        Ok(count)
    }

    /// Get statistics
    pub fn stats(&self) -> SetStats {
        let mut stats = self.stats.read().clone();

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
}
