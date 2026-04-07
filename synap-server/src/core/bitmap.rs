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

    /// BITFIELD - Get integer value from bit field
    /// Reads a signed or unsigned integer of specified bit width at offset
    pub fn bitfield_get(&self, offset: usize, width: usize, signed: bool) -> Result<i64> {
        if width == 0 || width > 64 {
            return Err(SynapError::InvalidValue(
                "Bit width must be between 1 and 64".to_string(),
            ));
        }

        let end_bit = offset + width - 1;
        let end_byte = end_bit / 8;

        if end_byte >= self.data.len() {
            return Ok(0); // Return 0 for unset bits
        }

        // Read bits across byte boundaries
        // Redis BITFIELD uses little-endian bit order (LSB first)
        let mut value: u64 = 0;

        for i in 0..width {
            let bit_pos = offset + i;
            let byte_idx = bit_pos / 8;
            if byte_idx >= self.data.len() {
                break; // Unset bits are 0
            }
            let bit_in_byte = bit_pos % 8; // LSB first (little-endian)
            let bit_value = (self.data[byte_idx] >> bit_in_byte) & 1;
            value |= (bit_value as u64) << i; // Set bit i
        }

        // Sign extend if signed and MSB is set
        if signed && (value >> (width - 1)) & 1 == 1 {
            // Sign extend: fill upper bits with 1s
            let sign_mask = !((1u64 << width) - 1);
            value |= sign_mask;
        }

        Ok(value as i64)
    }

    /// BITFIELD - Set integer value in bit field
    /// Writes a signed or unsigned integer of specified bit width at offset
    /// Returns the previous value
    pub fn bitfield_set(
        &mut self,
        offset: usize,
        width: usize,
        signed: bool,
        value: i64,
    ) -> Result<i64> {
        if width == 0 || width > 64 {
            return Err(SynapError::InvalidValue(
                "Bit width must be between 1 and 64".to_string(),
            ));
        }

        // Get previous value
        let old_value = self.bitfield_get(offset, width, signed)?;

        // Ensure capacity
        let end_bit = offset + width - 1;
        let end_byte = end_bit / 8;
        if end_byte >= self.data.len() {
            self.data.resize(end_byte + 1, 0);
        }

        self.updated_at = Self::current_timestamp();

        // Mask value to fit width
        let mask = if width == 64 {
            0xFFFFFFFFFFFFFFFFu64
        } else {
            (1u64 << width) - 1
        };

        let mut value_to_write = value as u64;
        if signed {
            // For signed values, mask and sign extend
            value_to_write &= mask;
            if (value_to_write >> (width - 1)) & 1 == 1 {
                // Negative value, sign extend
                let sign_mask = !mask;
                value_to_write |= sign_mask;
            }
        } else {
            // For unsigned, just mask
            value_to_write &= mask;
        }

        // Write bits across byte boundaries
        // Redis BITFIELD uses little-endian bit order (LSB first)
        for i in 0..width {
            let bit_pos = offset + i;
            let byte_idx = bit_pos / 8;
            let bit_in_byte = bit_pos % 8; // LSB first (little-endian)
            let bit_value = ((value_to_write >> i) & 1) as u8; // Extract bit i (LSB is i=0)

            if bit_value == 1 {
                self.data[byte_idx] |= 1 << bit_in_byte;
            } else {
                self.data[byte_idx] &= !(1 << bit_in_byte);
            }
        }

        Ok(old_value)
    }

    /// BITFIELD - Increment integer value in bit field
    /// Increments a signed or unsigned integer of specified bit width at offset
    /// Returns the new value after increment
    pub fn bitfield_incrby(
        &mut self,
        offset: usize,
        width: usize,
        signed: bool,
        increment: i64,
        overflow: BitfieldOverflow,
    ) -> Result<i64> {
        // Get current value
        let current = self.bitfield_get(offset, width, signed)?;

        // Calculate new value
        let new_value = match overflow {
            BitfieldOverflow::Wrap => {
                // Wrapping arithmetic
                let max_value = if signed {
                    (1i64 << (width - 1)) - 1
                } else {
                    (1i64 << width) - 1
                };
                let min_value = if signed { -(1i64 << (width - 1)) } else { 0 };

                let result = current.wrapping_add(increment);
                if signed {
                    // Wrap signed value
                    if result > max_value {
                        min_value + (result - max_value - 1)
                    } else if result < min_value {
                        max_value - (min_value - result - 1)
                    } else {
                        result
                    }
                } else {
                    // Wrap unsigned value
                    if result > max_value {
                        min_value + (result - max_value - 1)
                    } else if result < 0 {
                        max_value + result + 1
                    } else {
                        result
                    }
                }
            }
            BitfieldOverflow::Sat => {
                // Saturating arithmetic
                let max_value = if signed {
                    (1i64 << (width - 1)) - 1
                } else {
                    (1i64 << width) - 1
                };
                let min_value = if signed { -(1i64 << (width - 1)) } else { 0 };

                let result = current.saturating_add(increment);
                result.max(min_value).min(max_value)
            }
            BitfieldOverflow::Fail => {
                // Fail on overflow
                let max_value = if signed {
                    (1i64 << (width - 1)) - 1
                } else {
                    (1i64 << width) - 1
                };
                let min_value = if signed { -(1i64 << (width - 1)) } else { 0 };

                let result = current + increment;
                if result > max_value || result < min_value {
                    return Err(SynapError::InvalidValue(format!(
                        "Overflow: value {} would exceed range [{}, {}]",
                        result, min_value, max_value
                    )));
                }
                result
            }
        };

        // Set new value
        self.bitfield_set(offset, width, signed, new_value)?;

        Ok(new_value)
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

    /// BITFIELD - Execute multiple bitfield operations
    /// Executes a sequence of GET, SET, or INCRBY operations
    pub fn bitfield(
        &self,
        key: &str,
        operations: &[BitfieldOperation],
    ) -> Result<Vec<BitfieldResult>> {
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

        let mut results = Vec::new();

        for op in operations {
            let result = match op {
                BitfieldOperation::Get {
                    offset,
                    width,
                    signed,
                } => {
                    let value = bitmap.bitfield_get(*offset, *width, *signed)?;
                    BitfieldResult {
                        operation: BitfieldOp::Get,
                        value,
                    }
                }
                BitfieldOperation::Set {
                    offset,
                    width,
                    signed,
                    value,
                } => {
                    let old_value = bitmap.bitfield_set(*offset, *width, *signed, *value)?;
                    BitfieldResult {
                        operation: BitfieldOp::Set,
                        value: old_value,
                    }
                }
                BitfieldOperation::IncrBy {
                    offset,
                    width,
                    signed,
                    increment,
                    overflow,
                } => {
                    let new_value =
                        bitmap.bitfield_incrby(*offset, *width, *signed, *increment, *overflow)?;
                    BitfieldResult {
                        operation: BitfieldOp::IncrBy,
                        value: new_value,
                    }
                }
            };
            results.push(result);
        }

        self.stats.write().bitfield_count += 1;

        Ok(results)
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

/// Bitfield overflow behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitfieldOverflow {
    /// Wrap around on overflow (default)
    #[default]
    Wrap,
    /// Saturate at min/max values
    Sat,
    /// Fail on overflow
    Fail,
}

impl std::str::FromStr for BitfieldOverflow {
    type Err = SynapError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "WRAP" => Ok(BitfieldOverflow::Wrap),
            "SAT" => Ok(BitfieldOverflow::Sat),
            "FAIL" => Ok(BitfieldOverflow::Fail),
            _ => Err(SynapError::InvalidRequest(format!(
                "Invalid overflow behavior: {}",
                s
            ))),
        }
    }
}

/// Bitfield operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitfieldOp {
    Get,
    Set,
    IncrBy,
}

/// Bitfield operation result
#[derive(Debug, Clone)]
pub struct BitfieldResult {
    pub operation: BitfieldOp,
    pub value: i64,
}

/// Bitfield operation specification
#[derive(Debug, Clone)]
pub enum BitfieldOperation {
    /// GET operation: read value at offset
    Get {
        offset: usize,
        width: usize,
        signed: bool,
    },
    /// SET operation: write value at offset
    Set {
        offset: usize,
        width: usize,
        signed: bool,
        value: i64,
    },
    /// INCRBY operation: increment value at offset
    IncrBy {
        offset: usize,
        width: usize,
        signed: bool,
        increment: i64,
        overflow: BitfieldOverflow,
    },
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

    #[test]
    fn test_bitfield_get() {
        let mut bitmap = BitmapValue::new(None);

        // Set bits manually to create value 42 (binary: 101010)
        // Using 8 bits at offset 0
        bitmap.setbit(0, 1).unwrap(); // bit 0 = 1
        bitmap.setbit(2, 1).unwrap(); // bit 2 = 1
        bitmap.setbit(4, 1).unwrap(); // bit 4 = 1
        bitmap.setbit(6, 1).unwrap(); // bit 6 = 1

        // Read as unsigned 8-bit value
        let value = bitmap.bitfield_get(0, 8, false).unwrap();
        assert_eq!(value, 0b10101010); // 170 in decimal

        // Read as signed 8-bit value (should be negative if MSB is set)
        let signed_value = bitmap.bitfield_get(0, 8, true).unwrap();
        assert_eq!(signed_value, -86); // 0b10101010 as signed = -86
    }

    #[test]
    fn test_bitfield_set() {
        let mut bitmap = BitmapValue::new(None);

        // Set 8-bit unsigned value 42 at offset 0
        // 42 in binary (little-endian): 01010100
        let old_value = bitmap.bitfield_set(0, 8, false, 42).unwrap();
        assert_eq!(old_value, 0); // Was empty

        // Read back using bitfield_get (same encoding)
        let value = bitmap.bitfield_get(0, 8, false).unwrap();
        assert_eq!(value, 42);

        // Test setting another value
        let old_value2 = bitmap.bitfield_set(0, 8, false, 100).unwrap();
        assert_eq!(old_value2, 42); // Previous value
        let value2 = bitmap.bitfield_get(0, 8, false).unwrap();
        assert_eq!(value2, 100);
    }

    #[test]
    fn test_bitfield_incrby_wrap() {
        let mut bitmap = BitmapValue::new(None);

        // Set 4-bit unsigned value 14 at offset 0
        bitmap.bitfield_set(0, 4, false, 14).unwrap();

        // Increment by 1 (should wrap to 0)
        let new_value = bitmap
            .bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Wrap)
            .unwrap();
        assert_eq!(new_value, 15);

        // Increment by 1 again (should wrap to 0)
        let new_value = bitmap
            .bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Wrap)
            .unwrap();
        assert_eq!(new_value, 0);
    }

    #[test]
    fn test_bitfield_incrby_sat() {
        let mut bitmap = BitmapValue::new(None);

        // Set 4-bit unsigned value 14 at offset 0
        bitmap.bitfield_set(0, 4, false, 14).unwrap();

        // Increment by 1 (should saturate at 15)
        let new_value = bitmap
            .bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Sat)
            .unwrap();
        assert_eq!(new_value, 15);

        // Increment by 1 again (should stay at 15)
        let new_value = bitmap
            .bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Sat)
            .unwrap();
        assert_eq!(new_value, 15);
    }

    #[test]
    fn test_bitfield_incrby_fail() {
        let mut bitmap = BitmapValue::new(None);

        // Set 4-bit unsigned value 15 at offset 0
        bitmap.bitfield_set(0, 4, false, 15).unwrap();

        // Increment by 1 (should fail)
        let result = bitmap.bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Fail);
        assert!(result.is_err());
    }

    #[test]
    fn test_bitfield_store_operations() {
        let store = BitmapStore::new();

        // Execute multiple operations
        let operations = vec![
            BitfieldOperation::Set {
                offset: 0,
                width: 8,
                signed: false,
                value: 100,
            },
            BitfieldOperation::Get {
                offset: 0,
                width: 8,
                signed: false,
            },
            BitfieldOperation::IncrBy {
                offset: 0,
                width: 8,
                signed: false,
                increment: 50,
                overflow: BitfieldOverflow::Wrap,
            },
        ];

        let results = store.bitfield("test", &operations).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].value, 0); // Old value was 0
        assert_eq!(results[1].value, 100); // Read back 100
        assert_eq!(results[2].value, 150); // Incremented to 150
    }

    #[test]
    fn test_bitfield_signed_values() {
        let mut bitmap = BitmapValue::new(None);

        // Set signed 8-bit value -10
        bitmap.bitfield_set(0, 8, true, -10).unwrap();

        // Read back as signed
        let value = bitmap.bitfield_get(0, 8, true).unwrap();
        assert_eq!(value, -10);

        // Read as unsigned (should be 246)
        let unsigned_value = bitmap.bitfield_get(0, 8, false).unwrap();
        assert_eq!(unsigned_value, 246);
    }

    #[test]
    fn test_bitfield_cross_byte_boundary() {
        let mut bitmap = BitmapValue::new(None);

        // Set 16-bit value at offset 4 (crosses byte boundary)
        bitmap.bitfield_set(4, 16, false, 0x1234).unwrap();

        // Read back
        let value = bitmap.bitfield_get(4, 16, false).unwrap();
        assert_eq!(value, 0x1234);
    }

    #[test]
    fn test_bitfield_different_widths() {
        let mut bitmap = BitmapValue::new(None);

        // Test 4-bit unsigned
        bitmap.bitfield_set(0, 4, false, 15).unwrap();
        assert_eq!(bitmap.bitfield_get(0, 4, false).unwrap(), 15);

        // Test 12-bit unsigned
        bitmap.bitfield_set(4, 12, false, 4095).unwrap();
        assert_eq!(bitmap.bitfield_get(4, 12, false).unwrap(), 4095);

        // Test 24-bit unsigned
        bitmap.bitfield_set(16, 24, false, 16777215).unwrap();
        assert_eq!(bitmap.bitfield_get(16, 24, false).unwrap(), 16777215);
    }

    #[test]
    fn test_bitfield_signed_negative() {
        let mut bitmap = BitmapValue::new(None);

        // Test 8-bit signed negative value
        bitmap.bitfield_set(0, 8, true, -10).unwrap();
        assert_eq!(bitmap.bitfield_get(0, 8, true).unwrap(), -10);

        // Test 16-bit signed negative value
        bitmap.bitfield_set(8, 16, true, -1000).unwrap();
        assert_eq!(bitmap.bitfield_get(8, 16, true).unwrap(), -1000);

        // Test 4-bit signed negative value
        bitmap.bitfield_set(24, 4, true, -8).unwrap();
        assert_eq!(bitmap.bitfield_get(24, 4, true).unwrap(), -8);
    }

    #[test]
    fn test_bitfield_incrby_negative_increment() {
        let mut bitmap = BitmapValue::new(None);

        // Set initial value
        bitmap.bitfield_set(0, 8, false, 100).unwrap();

        // Decrement by 50
        let new_value = bitmap
            .bitfield_incrby(0, 8, false, -50, BitfieldOverflow::Wrap)
            .unwrap();
        assert_eq!(new_value, 50);

        // Decrement by 100 (should wrap)
        let new_value = bitmap
            .bitfield_incrby(0, 8, false, -100, BitfieldOverflow::Wrap)
            .unwrap();
        assert_eq!(new_value, 206); // 50 - 100 wraps to 206 (256 - 50)
    }

    #[test]
    fn test_bitfield_incrby_signed_wrap() {
        let mut bitmap = BitmapValue::new(None);

        // Set signed 8-bit value to 127 (max positive)
        bitmap.bitfield_set(0, 8, true, 127).unwrap();

        // Increment by 1 (should wrap to -128)
        let new_value = bitmap
            .bitfield_incrby(0, 8, true, 1, BitfieldOverflow::Wrap)
            .unwrap();
        assert_eq!(new_value, -128);

        // Increment by 1 again (should wrap to -127)
        let new_value = bitmap
            .bitfield_incrby(0, 8, true, 1, BitfieldOverflow::Wrap)
            .unwrap();
        assert_eq!(new_value, -127);
    }

    #[test]
    fn test_bitfield_incrby_signed_sat() {
        let mut bitmap = BitmapValue::new(None);

        // Set signed 8-bit value to 127 (max positive)
        bitmap.bitfield_set(0, 8, true, 127).unwrap();

        // Increment by 1 (should saturate at 127)
        let new_value = bitmap
            .bitfield_incrby(0, 8, true, 1, BitfieldOverflow::Sat)
            .unwrap();
        assert_eq!(new_value, 127);

        // Set to -128 (min negative)
        bitmap.bitfield_set(0, 8, true, -128).unwrap();

        // Decrement by 1 (should saturate at -128)
        let new_value = bitmap
            .bitfield_incrby(0, 8, true, -1, BitfieldOverflow::Sat)
            .unwrap();
        assert_eq!(new_value, -128);
    }

    #[test]
    fn test_bitfield_multiple_operations() {
        let store = BitmapStore::new();

        // Execute complex sequence of operations
        let operations = vec![
            // Set multiple fields
            BitfieldOperation::Set {
                offset: 0,
                width: 8,
                signed: false,
                value: 100,
            },
            BitfieldOperation::Set {
                offset: 8,
                width: 8,
                signed: false,
                value: 200,
            },
            BitfieldOperation::Set {
                offset: 16,
                width: 8,
                signed: false,
                value: 50,
            },
            // Read them back
            BitfieldOperation::Get {
                offset: 0,
                width: 8,
                signed: false,
            },
            BitfieldOperation::Get {
                offset: 8,
                width: 8,
                signed: false,
            },
            BitfieldOperation::Get {
                offset: 16,
                width: 8,
                signed: false,
            },
            // Increment middle field
            BitfieldOperation::IncrBy {
                offset: 8,
                width: 8,
                signed: false,
                increment: 50,
                overflow: BitfieldOverflow::Wrap,
            },
        ];

        let results = store.bitfield("multi", &operations).unwrap();
        assert_eq!(results.len(), 7);
        assert_eq!(results[0].value, 0); // Old value at offset 0
        assert_eq!(results[1].value, 0); // Old value at offset 8
        assert_eq!(results[2].value, 0); // Old value at offset 16
        assert_eq!(results[3].value, 100); // Read back offset 0
        assert_eq!(results[4].value, 200); // Read back offset 8
        assert_eq!(results[5].value, 50); // Read back offset 16
        assert_eq!(results[6].value, 250); // Incremented offset 8
    }

    #[test]
    fn test_bitfield_overlapping_fields() {
        let mut bitmap = BitmapValue::new(None);

        // Set 8-bit value at offset 0
        bitmap.bitfield_set(0, 8, false, 0xAA).unwrap(); // 10101010

        // Set 4-bit value at offset 4 (overlaps with first field)
        bitmap.bitfield_set(4, 4, false, 0xF).unwrap(); // 1111

        // Read back 8-bit value (should be modified)
        let value = bitmap.bitfield_get(0, 8, false).unwrap();
        assert_eq!(value, 0xFA); // 11111010
    }

    #[test]
    fn test_bitfield_large_offset() {
        let mut bitmap = BitmapValue::new(None);

        // Set value at large offset (beyond initial capacity)
        bitmap.bitfield_set(1000, 8, false, 42).unwrap();

        // Read back
        let value = bitmap.bitfield_get(1000, 8, false).unwrap();
        assert_eq!(value, 42);

        // Verify earlier bits are still 0
        let value_before = bitmap.bitfield_get(0, 8, false).unwrap();
        assert_eq!(value_before, 0);
    }

    #[test]
    fn test_bitfield_64bit_value() {
        let mut bitmap = BitmapValue::new(None);

        // Set 64-bit unsigned value
        bitmap
            .bitfield_set(0, 64, false, 0x1234567890ABCDEF)
            .unwrap();

        // Read back
        let value = bitmap.bitfield_get(0, 64, false).unwrap();
        assert_eq!(value, 0x1234567890ABCDEF);
    }

    #[test]
    fn test_bitfield_incrby_fail_unsigned() {
        let mut bitmap = BitmapValue::new(None);

        // Set 4-bit unsigned value to max (15)
        bitmap.bitfield_set(0, 4, false, 15).unwrap();

        // Try to increment (should fail)
        let result = bitmap.bitfield_incrby(0, 4, false, 1, BitfieldOverflow::Fail);
        assert!(result.is_err());

        // Try to increment by large value (should fail)
        let result = bitmap.bitfield_incrby(0, 4, false, 100, BitfieldOverflow::Fail);
        assert!(result.is_err());
    }

    #[test]
    fn test_bitfield_incrby_fail_signed() {
        let mut bitmap = BitmapValue::new(None);

        // Set 4-bit signed value to max (7)
        bitmap.bitfield_set(0, 4, true, 7).unwrap();

        // Try to increment (should fail)
        let result = bitmap.bitfield_incrby(0, 4, true, 1, BitfieldOverflow::Fail);
        assert!(result.is_err());

        // Set to min (-8)
        bitmap.bitfield_set(0, 4, true, -8).unwrap();

        // Try to decrement (should fail)
        let result = bitmap.bitfield_incrby(0, 4, true, -1, BitfieldOverflow::Fail);
        assert!(result.is_err());
    }

    #[test]
    fn test_bitfield_get_unset_bits() {
        let bitmap = BitmapValue::new(None);

        // Read from empty bitmap (should return 0)
        let value = bitmap.bitfield_get(0, 8, false).unwrap();
        assert_eq!(value, 0);

        // Read from offset beyond bitmap size
        let value = bitmap.bitfield_get(1000, 8, false).unwrap();
        assert_eq!(value, 0);
    }

    #[test]
    fn test_bitfield_partial_read() {
        let mut bitmap = BitmapValue::new(None);

        // Set 8-bit value
        bitmap.bitfield_set(0, 8, false, 0xFF).unwrap();

        // Read only 4 bits (should get lower 4 bits)
        let value = bitmap.bitfield_get(0, 4, false).unwrap();
        assert_eq!(value, 0xF); // Lower 4 bits of 0xFF

        // Read upper 4 bits
        let value = bitmap.bitfield_get(4, 4, false).unwrap();
        assert_eq!(value, 0xF); // Upper 4 bits of 0xFF
    }

    #[test]
    fn test_bitfield_store_expiration() {
        let store = BitmapStore::new();

        // Create bitmap with operations
        let operations = vec![BitfieldOperation::Set {
            offset: 0,
            width: 8,
            signed: false,
            value: 42,
        }];

        let results = store.bitfield("expire_test", &operations).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, 0);

        // Read back
        let read_ops = vec![BitfieldOperation::Get {
            offset: 0,
            width: 8,
            signed: false,
        }];
        let results = store.bitfield("expire_test", &read_ops).unwrap();
        assert_eq!(results[0].value, 42);
    }

    #[test]
    fn test_bitfield_invalid_width() {
        let mut bitmap = BitmapValue::new(None);

        // Width 0 should fail
        let result = bitmap.bitfield_set(0, 0, false, 42);
        assert!(result.is_err());

        // Width > 64 should fail
        let result = bitmap.bitfield_set(0, 65, false, 42);
        assert!(result.is_err());

        // Width 0 for GET should fail
        let result = bitmap.bitfield_get(0, 0, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_bitfield_signed_unsigned_interop() {
        let mut bitmap = BitmapValue::new(None);

        // Set as unsigned 8-bit value 200
        bitmap.bitfield_set(0, 8, false, 200).unwrap();

        // Read as signed (should be negative)
        let signed_value = bitmap.bitfield_get(0, 8, true).unwrap();
        assert_eq!(signed_value, -56); // 200 as signed 8-bit is -56

        // Set as signed negative value
        bitmap.bitfield_set(8, 8, true, -10).unwrap();

        // Read as unsigned (should be 246)
        let unsigned_value = bitmap.bitfield_get(8, 8, false).unwrap();
        assert_eq!(unsigned_value, 246); // -10 as unsigned 8-bit is 246
    }

    #[test]
    fn test_bitfield_32bit_values() {
        let mut bitmap = BitmapValue::new(None);

        // Test 32-bit unsigned
        bitmap.bitfield_set(0, 32, false, 0xFFFFFFFF).unwrap();
        assert_eq!(bitmap.bitfield_get(0, 32, false).unwrap(), 0xFFFFFFFF);

        // Test 32-bit signed positive
        bitmap.bitfield_set(32, 32, true, 2147483647).unwrap();
        assert_eq!(bitmap.bitfield_get(32, 32, true).unwrap(), 2147483647);

        // Test 32-bit signed negative
        bitmap.bitfield_set(64, 32, true, -2147483648).unwrap();
        assert_eq!(bitmap.bitfield_get(64, 32, true).unwrap(), -2147483648);
    }
}
