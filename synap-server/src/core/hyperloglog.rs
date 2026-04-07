//! HyperLogLog data structure implementation for Synap
//!
//! Provides Redis-compatible HyperLogLog operations (PFADD, PFCOUNT, PFMERGE)
//! Storage: Probabilistic cardinality estimation with ~0.81% error in ~12KB memory
//!
//! # Performance Targets
//! - PFADD: <200µs p99 latency
//! - PFCOUNT: <100µs p99 latency
//! - PFMERGE: <1ms p99 latency
//!
//! # Architecture
//! ```text
//! HyperLogLogStore
//!   ├─ 64 shards (Arc<RwLock<HashMap<key, HyperLogLogValue>>>)
//!   └─ TTL applies to entire HLL
//! ```

use super::error::{Result, SynapError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::{DefaultHasher, Entry};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const SHARD_COUNT: usize = 64;
const HLL_P: u8 = 14; // Precision, 2^P registers = 16384 registers
const HLL_REGISTER_COUNT: usize = 1 << HLL_P; // 16384

/// HyperLogLog register value (0-64)
type Register = u8;

/// HyperLogLog value stored in a single key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLogLogValue {
    /// Registers for cardinality estimation
    pub registers: Vec<Register>,
    /// TTL for entire HLL
    pub ttl_secs: Option<u64>,
    /// Creation timestamp
    pub created_at: u64,
    /// Last modification timestamp
    pub updated_at: u64,
}

impl HyperLogLogValue {
    /// Create new HyperLogLog value
    pub fn new(ttl_secs: Option<u64>) -> Self {
        let now = Self::current_timestamp();
        Self {
            registers: vec![0; HLL_REGISTER_COUNT],
            ttl_secs,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update TTL configuration and reset TTL timer
    pub fn set_ttl(&mut self, ttl_secs: Option<u64>) {
        self.ttl_secs = ttl_secs;
        let now = Self::current_timestamp();
        self.created_at = now;
        self.updated_at = now;
    }

    /// Get current Unix timestamp
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Check if HLL has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_secs {
            let now = Self::current_timestamp();
            now >= self.created_at + ttl
        } else {
            false
        }
    }

    /// Add element(s) to HyperLogLog
    /// Returns number of elements that were actually new (approximate)
    pub fn pfadd(&mut self, elements: Vec<Vec<u8>>) -> usize {
        self.updated_at = Self::current_timestamp();
        let mut estimated_added = 0;

        for element in elements {
            let hash = self.hash_element(&element);
            let j = (hash & ((1 << HLL_P) - 1)) as usize; // Register index
            let w = hash >> HLL_P; // Remaining bits

            // Count leading zeros + 1
            let rho = self.leading_zeros(w) + 1;

            if self.registers[j] < rho {
                self.registers[j] = rho;
                estimated_added += 1;
            }
        }

        estimated_added
    }

    /// Estimate cardinality using HyperLogLog algorithm
    pub fn pfcount(&self) -> u64 {
        let mut sum = 0.0;
        let mut zero_registers = 0;

        for &reg in &self.registers {
            sum += 1.0 / (2.0f64).powi(reg as i32);
            if reg == 0 {
                zero_registers += 1;
            }
        }

        let alpha = self.get_alpha();
        let mut estimate = alpha * (HLL_REGISTER_COUNT as f64).powi(2) / sum;

        // Small range correction
        if estimate <= 5.0 * HLL_REGISTER_COUNT as f64 / 2.0 && zero_registers > 0 {
            estimate = HLL_REGISTER_COUNT as f64
                * (HLL_REGISTER_COUNT as f64 / zero_registers as f64).ln();
        }

        // Large range correction
        if estimate > (1u64 << 32) as f64 / 30.0 {
            estimate = -((1u64 << 32) as f64) * (1.0 - estimate / (1u64 << 32) as f64).ln();
        }

        estimate as u64
    }

    /// Merge another HyperLogLog into this one
    pub fn merge(&mut self, other: &HyperLogLogValue) {
        self.updated_at = Self::current_timestamp();
        for i in 0..HLL_REGISTER_COUNT {
            if other.registers[i] > self.registers[i] {
                self.registers[i] = other.registers[i];
            }
        }
    }

    /// Hash element to u64
    fn hash_element(&self, element: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        element.hash(&mut hasher);
        hasher.finish()
    }

    /// Count leading zeros in a u64 (after extracting register index)
    /// The value passed here is already right-shifted by P bits
    fn leading_zeros(&self, value: u64) -> u8 {
        if value == 0 {
            // All remaining bits are zero, return max
            return 64 - HLL_P;
        }
        value.leading_zeros() as u8
    }

    /// Get alpha constant for cardinality estimation
    fn get_alpha(&self) -> f64 {
        match HLL_P {
            4 => 0.673,
            5 => 0.697,
            6 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / HLL_REGISTER_COUNT as f64),
        }
    }
}

/// Statistics for HyperLogLog operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HyperLogLogStats {
    pub total_hlls: usize,
    pub pfadd_count: u64,
    pub pfcount_count: u64,
    pub pfmerge_count: u64,
    pub total_cardinality: u64,
}

impl HyperLogLogStats {
    /// Reset statistics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// HyperLogLog store with sharding
pub struct HyperLogLogStore {
    shards: Vec<Arc<RwLock<HashMap<String, HyperLogLogValue>>>>,
    pub stats: Arc<RwLock<HyperLogLogStats>>,
}

impl Default for HyperLogLogStore {
    fn default() -> Self {
        Self::new()
    }
}

impl HyperLogLogStore {
    /// Create new HyperLogLog store
    pub fn new() -> Self {
        let mut shards = Vec::with_capacity(SHARD_COUNT);
        for _ in 0..SHARD_COUNT {
            shards.push(Arc::new(RwLock::new(HashMap::new())));
        }

        Self {
            shards,
            stats: Arc::new(RwLock::new(HyperLogLogStats::default())),
        }
    }

    /// Get shard index for key
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % SHARD_COUNT
    }

    /// Get shard for key
    fn shard(&self, key: &str) -> &Arc<RwLock<HashMap<String, HyperLogLogValue>>> {
        &self.shards[self.shard_index(key)]
    }

    /// PFADD - Add element(s) to HyperLogLog
    pub fn pfadd(&self, key: &str, elements: Vec<Vec<u8>>, ttl_secs: Option<u64>) -> Result<usize> {
        let shard = self.shard(key);
        let mut map = shard.write();

        // Remove expired value if present
        if let Some(existing_hll) = map.get(key) {
            if existing_hll.is_expired() {
                map.remove(key);
            }
        }

        let added = match map.entry(key.to_string()) {
            Entry::Occupied(mut entry) => {
                let hll = entry.get_mut();
                if let Some(ttl) = ttl_secs {
                    hll.set_ttl(Some(ttl));
                }
                hll.pfadd(elements)
            }
            Entry::Vacant(vacant) => {
                let mut hll = HyperLogLogValue::new(ttl_secs);
                let added = hll.pfadd(elements);
                vacant.insert(hll);
                added
            }
        };

        self.stats.write().pfadd_count += 1;

        Ok(added)
    }

    /// PFCOUNT - Estimate cardinality of HyperLogLog
    pub fn pfcount(&self, key: &str) -> Result<u64> {
        let shard = self.shard(key);
        let map = shard.read();

        let hll = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if hll.is_expired() {
            drop(map);
            let mut map = shard.write();
            map.remove(key);
            return Err(SynapError::KeyExpired);
        }

        let count = hll.pfcount();
        self.stats.write().pfcount_count += 1;

        Ok(count)
    }

    /// PFMERGE - Merge multiple HyperLogLogs into destination
    pub fn pfmerge(&self, dest_key: &str, source_keys: Vec<String>) -> Result<u64> {
        let dest_shard = self.shard(dest_key);
        let mut dest_map = dest_shard.write();

        // Create or get destination HLL
        let dest_hll = dest_map
            .entry(dest_key.to_string())
            .or_insert_with(|| HyperLogLogValue::new(None));

        if dest_hll.is_expired() {
            let ttl = dest_hll.ttl_secs;
            *dest_hll = HyperLogLogValue::new(ttl);
        }

        // Collect source HLLs
        let mut source_hlls = Vec::new();
        for source_key in &source_keys {
            if source_key == dest_key {
                continue; // Skip self
            }

            let source_shard = self.shard(source_key);
            let source_map = source_shard.read();

            if let Some(source_hll) = source_map.get(source_key) {
                if !source_hll.is_expired() {
                    source_hlls.push(source_hll.clone());
                }
            }
        }

        // Merge all sources into destination
        for source_hll in source_hlls {
            dest_hll.merge(&source_hll);
        }

        let count = dest_hll.pfcount();
        self.stats.write().pfmerge_count += 1;

        Ok(count)
    }

    /// Get statistics
    pub fn stats(&self) -> HyperLogLogStats {
        let mut stats = self.stats.read().clone();

        let mut total_hlls = 0usize;
        let mut total_cardinality = 0u64;

        for shard in &self.shards {
            let map = shard.read();
            for value in map.values() {
                if value.is_expired() {
                    continue;
                }

                total_hlls += 1;
                total_cardinality = total_cardinality.saturating_add(value.pfcount());
            }
        }

        stats.total_hlls = total_hlls;
        stats.total_cardinality = total_cardinality;

        stats
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.write().reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn pfadd_and_pfcount_work() {
        let store = HyperLogLogStore::new();

        let added = store
            .pfadd(
                "visitors",
                vec![b"user:1".to_vec(), b"user:2".to_vec(), b"user:3".to_vec()],
                None,
            )
            .unwrap();

        assert!(added >= 1);

        let count = store.pfcount("visitors").unwrap();
        assert!(count >= 3);
    }

    #[test]
    fn pfmerge_merges_sources() {
        let store = HyperLogLogStore::new();

        store
            .pfadd("source:a", vec![b"alpha".to_vec(), b"beta".to_vec()], None)
            .unwrap();
        store
            .pfadd("source:b", vec![b"gamma".to_vec(), b"delta".to_vec()], None)
            .unwrap();

        let merged = store
            .pfmerge("dest", vec!["source:a".into(), "source:b".into()])
            .unwrap();

        assert!(merged >= 3);
    }

    #[test]
    fn pfadd_respects_ttl() {
        let store = HyperLogLogStore::new();

        store
            .pfadd("temp", vec![b"user:1".to_vec()], Some(1))
            .unwrap();

        // Should be accessible immediately
        assert!(store.pfcount("temp").is_ok());

        thread::sleep(Duration::from_secs(2));

        let result = store.pfcount("temp");
        assert!(matches!(
            result,
            Err(SynapError::KeyExpired) | Err(SynapError::NotFound)
        ));
    }

    #[test]
    fn pfadd_duplicates_return_lower_count() {
        let store = HyperLogLogStore::new();

        // First add
        let added1 = store
            .pfadd("dups", vec![b"user:1".to_vec(), b"user:2".to_vec()], None)
            .unwrap();
        assert!(added1 >= 1);

        // Add same elements again
        let added2 = store
            .pfadd("dups", vec![b"user:1".to_vec(), b"user:2".to_vec()], None)
            .unwrap();
        // Should add fewer (or zero) elements since they're duplicates
        assert!(added2 <= added1);

        // Count should remain similar
        let count = store.pfcount("dups").unwrap();
        assert!(count >= 2);
    }

    #[test]
    fn pfcount_nonexistent_key_returns_error() {
        let store = HyperLogLogStore::new();

        let result = store.pfcount("nonexistent");
        assert!(matches!(result, Err(SynapError::NotFound)));
    }

    #[test]
    fn pfadd_empty_elements() {
        let store = HyperLogLogStore::new();

        let added = store.pfadd("empty", vec![], None).unwrap();
        assert_eq!(added, 0);

        // Empty HLL should have count 0 or very small
        let count = store.pfcount("empty").unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn pfadd_large_set() {
        let store = HyperLogLogStore::new();

        let mut elements = Vec::new();
        for i in 0..1000 {
            elements.push(format!("user:{}", i).into_bytes());
        }

        let added = store.pfadd("large", elements, None).unwrap();
        assert!(added >= 100);

        let count = store.pfcount("large").unwrap();
        // Should be approximately 1000, but with some error tolerance
        assert!((800..=1200).contains(&count));
    }

    #[test]
    fn pfmerge_with_empty_source() {
        let store = HyperLogLogStore::new();

        // Create source with data
        store
            .pfadd("source", vec![b"alpha".to_vec(), b"beta".to_vec()], None)
            .unwrap();

        // Merge with non-existent source (should be treated as empty)
        let merged = store
            .pfmerge("dest", vec!["source".into(), "nonexistent".into()])
            .unwrap();

        assert!(merged >= 2);
    }

    #[test]
    fn pfmerge_multiple_sources() {
        let store = HyperLogLogStore::new();

        store
            .pfadd("src1", vec![b"a".to_vec(), b"b".to_vec()], None)
            .unwrap();
        store
            .pfadd("src2", vec![b"c".to_vec(), b"d".to_vec()], None)
            .unwrap();
        store
            .pfadd("src3", vec![b"e".to_vec(), b"f".to_vec()], None)
            .unwrap();

        let merged = store
            .pfmerge("dest", vec!["src1".into(), "src2".into(), "src3".into()])
            .unwrap();

        assert!(merged >= 5);
    }

    #[test]
    fn pfmerge_self_reference_ignored() {
        let store = HyperLogLogStore::new();

        store
            .pfadd("self", vec![b"alpha".to_vec(), b"beta".to_vec()], None)
            .unwrap();

        // Merge with self (should be ignored)
        let merged = store.pfmerge("self", vec!["self".into()]).unwrap();

        // Should still have original count
        assert!(merged >= 2);
    }

    #[test]
    fn pfmerge_creates_destination_if_missing() {
        let store = HyperLogLogStore::new();

        store.pfadd("source", vec![b"data".to_vec()], None).unwrap();

        // Merge to non-existent destination
        let merged = store.pfmerge("new_dest", vec!["source".into()]).unwrap();

        assert!(merged >= 1);

        // Destination should now exist
        let count = store.pfcount("new_dest").unwrap();
        assert!(count >= 1);
    }

    #[test]
    fn stats_track_operations() {
        let store = HyperLogLogStore::new();

        store.pfadd("key1", vec![b"a".to_vec()], None).unwrap();
        store.pfadd("key2", vec![b"b".to_vec()], None).unwrap();
        store.pfcount("key1").unwrap();
        store
            .pfmerge("merged", vec!["key1".into(), "key2".into()])
            .unwrap();

        let stats = store.stats();

        assert_eq!(stats.total_hlls, 3); // key1, key2, merged
        assert_eq!(stats.pfadd_count, 2);
        assert_eq!(stats.pfcount_count, 1);
        assert_eq!(stats.pfmerge_count, 1);
        assert!(stats.total_cardinality > 0);
    }

    #[test]
    fn pfadd_updates_existing_ttl() {
        let store = HyperLogLogStore::new();

        // Create with TTL
        store
            .pfadd("ttl_key", vec![b"data".to_vec()], Some(60))
            .unwrap();

        // Update with new TTL
        store
            .pfadd("ttl_key", vec![b"more_data".to_vec()], Some(120))
            .unwrap();

        // Should still be accessible
        assert!(store.pfcount("ttl_key").is_ok());
    }

    #[test]
    fn pfadd_incremental_updates_cardinality() {
        let store = HyperLogLogStore::new();

        let mut count1 = 0;
        for i in 0..10 {
            store
                .pfadd(
                    "incremental",
                    vec![format!("item:{}", i).into_bytes()],
                    None,
                )
                .unwrap();
            count1 = store.pfcount("incremental").unwrap();
        }

        assert!((8..=12).contains(&count1)); // Approximate, with error tolerance

        // Add more items
        for i in 10..50 {
            store
                .pfadd(
                    "incremental",
                    vec![format!("item:{}", i).into_bytes()],
                    None,
                )
                .unwrap();
        }

        let count2 = store.pfcount("incremental").unwrap();
        assert!(count2 > count1);
        assert!((40..=60).contains(&count2)); // Approximate
    }

    #[test]
    fn pfmerge_preserves_destination_data() {
        let store = HyperLogLogStore::new();

        // Create destination with data
        store
            .pfadd("dest", vec![b"dest_data".to_vec()], None)
            .unwrap();

        // Create source
        store
            .pfadd("source", vec![b"source_data".to_vec()], None)
            .unwrap();

        // Merge source into destination
        let merged = store.pfmerge("dest", vec!["source".into()]).unwrap();

        // Should have both elements
        assert!(merged >= 2);
    }

    #[test]
    fn reset_stats_clears_counters() {
        let store = HyperLogLogStore::new();

        store.pfadd("key", vec![b"data".to_vec()], None).unwrap();
        store.pfcount("key").unwrap();

        let stats_before = store.stats();
        assert!(stats_before.pfadd_count > 0);

        store.reset_stats();

        let stats_after = store.stats();
        assert_eq!(stats_after.pfadd_count, 0);
        assert_eq!(stats_after.pfcount_count, 0);
        assert_eq!(stats_after.pfmerge_count, 0);
    }
}
