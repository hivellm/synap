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
use std::collections::hash_map::DefaultHasher;
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
    pub fn pfadd(&self, key: &str, elements: Vec<Vec<u8>>) -> Result<usize> {
        let shard = self.shard(key);
        let mut map = shard.write();

        // Check expiration first
        if let Some(existing_hll) = map.get(key) {
            if existing_hll.is_expired() {
                map.remove(key);
            }
        }

        let hll = map
            .entry(key.to_string())
            .or_insert_with(|| HyperLogLogValue::new(None));

        let added = hll.pfadd(elements);
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
        self.stats.read().clone()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.write().reset();
    }
}
