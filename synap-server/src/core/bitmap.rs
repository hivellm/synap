//! Bitmap data structure implementation for Synap
//!
//! Provides Redis-compatible bitmap operations (SETBIT, GETBIT, BITCOUNT, BITOP, BITPOS, BITFIELD)
//! Storage: Vec<u8> for efficient bit-level operations
//!
//! # Performance Targets
//! - SETBIT: <100µs p99 latency
//! - GETBIT: <50µs p99 latency
//! - BITCOUNT: <200µs p99 latency (for 1KB bitmap)
//! - BITOP: <500µs p99 latency (for two 1KB bitmaps)
//!
//! # Architecture
//! ```text
//! BitmapStore
//!   ├─ 64 shards (Arc<RwLock<HashMap<key, BitmapValue>>>)
//!   └─ TTL applies to entire bitmap
//! ```

use super::error::{Result, SynapError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const SHARD_COUNT: usize = 64;

/// Bitmap value stored in a single key
/// Uses Vec<u8> to store bits, where each byte contains 8 bits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitmapValue {
    /// Bits stored as bytes (each byte = 8 bits)
    pub data: Vec<u8>,
    /// TTL for entire bitmap (in seconds)
    pub ttl_secs: Option<u64>,
    /// Creation timestamp
    pub created_at: u64,
    /// Last modification timestamp
    pub updated_at: u64,
}

impl BitmapValue {
    /// Create new bitmap value
    pub fn new(ttl_secs: Option<u64>) -> Self {
        let now = Self::current_timestamp();
        Self {
            data: Vec::new(),
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

    /// Check if bitmap has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_secs {
            let now = Self::current_timestamp();
            now >= self.created_at + ttl
        } else {
            false
        }
    }

    /// Ensure bitmap is large enough for the given bit offset
    fn ensure_capacity(&mut self, offset: usize) {
        let byte_index = offset / 8;
        if byte_index >= self.data.len() {
            self.data.resize(byte_index + 1, 0);
        }
    }

    /// SETBIT - Set bit at offset to value (0 or 1)
    /// Returns the previous bit value
    pub fn setbit(&mut self, offset: usize, value: u8) -> Result<u8> {
        if value != 0 && value != 1 {
            return Err(SynapError::InvalidValue(
                "Bit value must be 0 or 1".to_string(),
            ));
        }

        self.ensure_capacity(offset);
        self.updated_at = Self::current_timestamp();

        let byte_index = offset / 8;
        let bit_index = 7 - (offset % 8); // MSB first (big-endian)

        let byte = self.data[byte_index];
        let old_bit = (byte >> bit_index) & 1;

        if value == 1 {
            self.data[byte_index] = byte | (1 << bit_index);
        } else {
            self.data[byte_index] = byte & !(1 << bit_index);
        }

        Ok(old_bit)
    }

    /// GETBIT - Get bit at offset
    pub fn getbit(&self, offset: usize) -> u8 {
        let byte_index = offset / 8;
        if byte_index >= self.data.len() {
            return 0;
        }

        let bit_index = 7 - (offset % 8); // MSB first (big-endian)
        (self.data[byte_index] >> bit_index) & 1
    }

    /// BITCOUNT - Count set bits in bitmap
    /// Optionally count in a range [start, end]
    pub fn bitcount(&self, start: Option<usize>, end: Option<usize>) -> usize {
        if self.data.is_empty() {
            return 0;
        }

        let start_offset = start.unwrap_or(0);
        let end_offset = end.unwrap_or(self.data.len() * 8 - 1);

        if start_offset > end_offset {
            return 0;
        }

        let start_byte = start_offset / 8;
        let end_byte = end_offset / 8;

        if start_byte >= self.data.len() {
            return 0;
        }

        let actual_end_byte = end_byte.min(self.data.len().saturating_sub(1));
        let mut count = 0;

        // Count full bytes
        for i in start_byte..=actual_end_byte {
            count += self.data[i].count_ones() as usize;
        }

        // Adjust for partial byte at start
        if let Some(start_bit) = start {
            let start_byte_idx = start_bit / 8;
            let start_bit_in_byte = start_bit % 8;
            if start_byte_idx < self.data.len() && start_bit_in_byte > 0 {
                // Clear bits before start_bit_in_byte
                let bits_to_clear = start_bit_in_byte;
                let byte = self.data[start_byte_idx];
                for bit_pos in 0..bits_to_clear {
                    let bit_index = 7 - bit_pos; // MSB first
                    if (byte >> bit_index) & 1 == 1 {
                        count -= 1;
                    }
                }
            }
        }

        // Adjust for partial byte at end
        if let Some(end_bit) = end {
            let end_byte_idx = end_bit / 8;
            let end_bit_in_byte = end_bit % 8;
            if end_byte_idx < self.data.len() && end_bit_in_byte < 7 {
                // Clear bits after end_bit_in_byte
                let byte = self.data[end_byte_idx];
                for bit_pos in (end_bit_in_byte + 1)..8 {
                    let bit_index = 7 - bit_pos; // MSB first
                    if (byte >> bit_index) & 1 == 1 {
                        count -= 1;
                    }
                }
            }
        }

        count.max(0) // Ensure non-negative
    }

    /// BITPOS - Find first bit set to value (0 or 1) starting from offset
    /// Returns Some(bit_offset) if found, None if not found
    pub fn bitpos(&self, value: u8, start: Option<usize>, end: Option<usize>) -> Option<usize> {
        if value != 0 && value != 1 {
            return None;
        }

        let start_offset = start.unwrap_or(0);
        let end_offset = end.unwrap_or(self.data.len() * 8 - 1);

        for offset in start_offset..=end_offset {
            let bit = self.getbit(offset);
            if bit == value {
                return Some(offset);
            }
        }

        None
    }

    /// Get bitmap length in bytes
    pub fn len_bytes(&self) -> usize {
        self.data.len()
    }

    /// Check if bitmap is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty() || self.data.iter().all(|&b| b == 0)
    }
}

impl Default for BitmapValue {
    fn default() -> Self {
        Self::new(None)
    }
}

/// Statistics for bitmap operations
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BitmapStats {
    pub total_bitmaps: usize,
    pub total_bits: usize,
    pub setbit_count: usize,
    pub getbit_count: usize,
    pub bitcount_count: usize,
    pub bitop_count: usize,
    pub bitpos_count: usize,
    pub bitfield_count: usize,
}

/// Bitmap store with sharded storage
pub struct BitmapStore {
    /// Sharded storage (64 shards for concurrent access)
    shards: Vec<Arc<RwLock<HashMap<String, BitmapValue>>>>,
    /// Statistics
    stats: Arc<RwLock<BitmapStats>>,
}

impl Default for BitmapStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BitmapStore {
    /// Create new bitmap store
    pub fn new() -> Self {
        let mut shards = Vec::with_capacity(SHARD_COUNT);
        for _ in 0..SHARD_COUNT {
            shards.push(Arc::new(RwLock::new(HashMap::new())));
        }

        Self {
            shards,
            stats: Arc::new(RwLock::new(BitmapStats::default())),
        }
    }

    /// Get shard index for key
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % SHARD_COUNT
    }

    /// Get shard for key
    fn shard(&self, key: &str) -> &Arc<RwLock<HashMap<String, BitmapValue>>> {
        &self.shards[self.shard_index(key)]
    }

    /// SETBIT - Set bit at offset to value (0 or 1)
    pub fn setbit(&self, key: &str, offset: usize, value: u8) -> Result<u8> {
        let shard = self.shard(key);
        let mut map = shard.write();

        // Remove expired value if present
        if let Some(existing_bitmap) = map.get(key) {
            if existing_bitmap.is_expired() {
                map.remove(key);
            }
        }

        let bitmap = map
            .entry(key.to_string())
            .or_insert_with(|| BitmapValue::new(None));

        let old_bit = bitmap.setbit(offset, value)?;
        self.stats.write().setbit_count += 1;

        Ok(old_bit)
    }

    /// GETBIT - Get bit at offset
    pub fn getbit(&self, key: &str, offset: usize) -> Result<u8> {
        let shard = self.shard(key);
        let map = shard.read();

        let bitmap = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if bitmap.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        self.stats.write().getbit_count += 1;
        Ok(bitmap.getbit(offset))
    }

    /// BITCOUNT - Count set bits in bitmap
    pub fn bitcount(&self, key: &str, start: Option<usize>, end: Option<usize>) -> Result<usize> {
        let shard = self.shard(key);
        let map = shard.read();

        let bitmap = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if bitmap.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        let count = bitmap.bitcount(start, end);
        self.stats.write().bitcount_count += 1;

        Ok(count)
    }

    /// BITPOS - Find first bit set to value starting from offset
    pub fn bitpos(
        &self,
        key: &str,
        value: u8,
        start: Option<usize>,
        end: Option<usize>,
    ) -> Result<Option<usize>> {
        if value != 0 && value != 1 {
            return Err(SynapError::InvalidValue(
                "Bit value must be 0 or 1".to_string(),
            ));
        }

        let shard = self.shard(key);
        let map = shard.read();

        let bitmap = map.get(key).ok_or(SynapError::NotFound)?;

        // Check expiration
        if bitmap.is_expired() {
            return Err(SynapError::KeyExpired);
        }

        let pos = bitmap.bitpos(value, start, end);
        self.stats.write().bitpos_count += 1;

        Ok(pos)
    }

    /// BITOP - Perform bitwise operation on multiple bitmaps
    /// Operations: AND, OR, XOR, NOT
    pub fn bitop(
        &self,
        operation: BitmapOperation,
        dest_key: &str,
        source_keys: &[String],
    ) -> Result<usize> {
        if source_keys.is_empty() {
            return Err(SynapError::InvalidRequest(
                "At least one source key required".to_string(),
            ));
        }

        // Get all source bitmaps
        let mut source_bitmaps = Vec::new();
        let mut max_len = 0;

        for source_key in source_keys {
            let shard = self.shard(source_key);
            let map = shard.read();

            if let Some(bitmap) = map.get(source_key) {
                if bitmap.is_expired() {
                    continue; // Skip expired bitmaps
                }
                max_len = max_len.max(bitmap.len_bytes());
                source_bitmaps.push(bitmap.data.clone());
            }
        }

        if source_bitmaps.is_empty() {
            return Err(SynapError::NotFound);
        }

        // Perform operation
        let result_data = match operation {
            BitmapOperation::And => {
                if source_bitmaps.len() < 2 {
                    return Err(SynapError::InvalidRequest(
                        "AND operation requires at least 2 source keys".to_string(),
                    ));
                }
                self.bitop_and(&source_bitmaps, max_len)
            }
            BitmapOperation::Or => {
                // OR can work with any number of sources (even empty, but we check above)
                self.bitop_or(&source_bitmaps, max_len)
            }
            BitmapOperation::Xor => {
                if source_bitmaps.len() < 2 {
                    return Err(SynapError::InvalidRequest(
                        "XOR operation requires at least 2 source keys".to_string(),
                    ));
                }
                self.bitop_xor(&source_bitmaps, max_len)
            }
            BitmapOperation::Not => {
                if source_bitmaps.len() != 1 {
                    return Err(SynapError::InvalidRequest(
                        "NOT operation requires exactly 1 source key".to_string(),
                    ));
                }
                self.bitop_not(&source_bitmaps[0], max_len)
            }
        };

        // Store result
        let dest_shard = self.shard(dest_key);
        let mut map = dest_shard.write();

        // Remove expired value if present
        if let Some(existing_bitmap) = map.get(dest_key) {
            if existing_bitmap.is_expired() {
                map.remove(dest_key);
            }
        }

        let dest_bitmap = map
            .entry(dest_key.to_string())
            .or_insert_with(|| BitmapValue::new(None));

        dest_bitmap.data = result_data;
        dest_bitmap.updated_at = BitmapValue::current_timestamp();

        let result_len = dest_bitmap.len_bytes();
        self.stats.write().bitop_count += 1;

        Ok(result_len)
    }

    /// Perform bitwise AND operation
    fn bitop_and(&self, bitmaps: &[Vec<u8>], max_len: usize) -> Vec<u8> {
        let mut result = vec![0xFF; max_len]; // Start with all 1s

        for bitmap in bitmaps {
            for (i, byte) in bitmap.iter().enumerate() {
                if i < result.len() {
                    result[i] &= byte;
                }
            }
        }

        result
    }

    /// Perform bitwise OR operation
    fn bitop_or(&self, bitmaps: &[Vec<u8>], max_len: usize) -> Vec<u8> {
        let mut result = vec![0; max_len];

        for bitmap in bitmaps {
            for (i, byte) in bitmap.iter().enumerate() {
                if i < result.len() {
                    result[i] |= byte;
                }
            }
        }

        result
    }

    /// Perform bitwise XOR operation
    fn bitop_xor(&self, bitmaps: &[Vec<u8>], max_len: usize) -> Vec<u8> {
        let mut result = if bitmaps.is_empty() {
            vec![0; max_len]
        } else {
            bitmaps[0].clone()
        };

        for bitmap in bitmaps.iter().skip(1) {
            for (i, byte) in bitmap.iter().enumerate() {
                if i < result.len() {
                    result[i] ^= byte;
                } else {
                    result.push(*byte);
                }
            }
        }

        result
    }

    /// Perform bitwise NOT operation
    fn bitop_not(&self, bitmap: &[u8], max_len: usize) -> Vec<u8> {
        let mut result = vec![0; max_len];

        for (i, byte) in bitmap.iter().enumerate() {
            if i < result.len() {
                result[i] = !byte;
            }
        }

        result
    }

    /// Get statistics
    pub fn stats(&self) -> BitmapStats {
        let mut stats = self.stats.read().clone();

        // Calculate total bitmaps and total bits
        let mut total_bitmaps = 0;
        let mut total_bits = 0;

        for shard in &self.shards {
            let map = shard.read();
            for bitmap in map.values() {
                if !bitmap.is_expired() {
                    total_bitmaps += 1;
                    total_bits += bitmap.len_bytes() * 8; // Approximate bits (bytes * 8)
                }
            }
        }

        stats.total_bitmaps = total_bitmaps;
        stats.total_bits = total_bits;

        stats
    }
}

/// Bitmap operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapOperation {
    And,
    Or,
    Xor,
    Not,
}

impl std::str::FromStr for BitmapOperation {
    type Err = SynapError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "AND" => Ok(BitmapOperation::And),
            "OR" => Ok(BitmapOperation::Or),
            "XOR" => Ok(BitmapOperation::Xor),
            "NOT" => Ok(BitmapOperation::Not),
            _ => Err(SynapError::InvalidRequest(format!(
                "Invalid bitmap operation: {}",
                s
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setbit_getbit() {
        let mut bitmap = BitmapValue::new(None);

        // Set bit 0 to 1
        assert_eq!(bitmap.setbit(0, 1).unwrap(), 0);
        assert_eq!(bitmap.getbit(0), 1);

        // Set bit 7 to 1 (same byte)
        assert_eq!(bitmap.setbit(7, 1).unwrap(), 0);
        assert_eq!(bitmap.getbit(7), 1);

        // Set bit 0 back to 0
        assert_eq!(bitmap.setbit(0, 0).unwrap(), 1);
        assert_eq!(bitmap.getbit(0), 0);

        // Set bit 8 to 1 (next byte)
        assert_eq!(bitmap.setbit(8, 1).unwrap(), 0);
        assert_eq!(bitmap.getbit(8), 1);
    }

    #[test]
    fn test_bitcount() {
        let mut bitmap = BitmapValue::new(None);

        // Set bits at positions 0, 2, 4, 6 (all in first byte)
        bitmap.setbit(0, 1).unwrap();
        bitmap.setbit(2, 1).unwrap();
        bitmap.setbit(4, 1).unwrap();
        bitmap.setbit(6, 1).unwrap();

        assert_eq!(bitmap.bitcount(None, None), 4);

        // Set more bits
        bitmap.setbit(8, 1).unwrap();
        bitmap.setbit(10, 1).unwrap();

        assert_eq!(bitmap.bitcount(None, None), 6);
    }

    #[test]
    fn test_bitpos() {
        let mut bitmap = BitmapValue::new(None);

        // Set bit at position 5
        bitmap.setbit(5, 1).unwrap();

        // Find first set bit
        assert_eq!(bitmap.bitpos(1, None, None), Some(5));

        // Find first unset bit (should be 0)
        assert_eq!(bitmap.bitpos(0, None, None), Some(0));

        // Find first set bit starting from position 6
        assert_eq!(bitmap.bitpos(1, Some(6), None), None);
    }

    #[test]
    fn test_bitop_and() {
        let store = BitmapStore::new();

        // Create two bitmaps
        store.setbit("bitmap1", 0, 1).unwrap();
        store.setbit("bitmap1", 1, 1).unwrap();
        store.setbit("bitmap1", 2, 1).unwrap();

        store.setbit("bitmap2", 1, 1).unwrap();
        store.setbit("bitmap2", 2, 1).unwrap();
        store.setbit("bitmap2", 3, 1).unwrap();

        // AND operation
        store
            .bitop(
                BitmapOperation::And,
                "result",
                &["bitmap1".to_string(), "bitmap2".to_string()],
            )
            .unwrap();

        assert_eq!(store.getbit("result", 0).unwrap(), 0);
        assert_eq!(store.getbit("result", 1).unwrap(), 1);
        assert_eq!(store.getbit("result", 2).unwrap(), 1);
        assert_eq!(store.getbit("result", 3).unwrap(), 0);
    }

    #[test]
    fn test_bitop_or() {
        let store = BitmapStore::new();

        store.setbit("bitmap1", 0, 1).unwrap();
        store.setbit("bitmap1", 1, 1).unwrap();

        store.setbit("bitmap2", 1, 1).unwrap();
        store.setbit("bitmap2", 2, 1).unwrap();

        store
            .bitop(
                BitmapOperation::Or,
                "result",
                &["bitmap1".to_string(), "bitmap2".to_string()],
            )
            .unwrap();

        assert_eq!(store.getbit("result", 0).unwrap(), 1);
        assert_eq!(store.getbit("result", 1).unwrap(), 1);
        assert_eq!(store.getbit("result", 2).unwrap(), 1);
        assert_eq!(store.getbit("result", 3).unwrap(), 0);
    }
}
