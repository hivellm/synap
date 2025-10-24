//! Hash data structure implementation for Synap
//!
//! Provides Redis-compatible hash operations (HSET, HGET, HDEL, etc.)
//! Storage: Nested HashMap within sharded KV store for O(1) field access
//!
//! # Performance Targets
//! - HSET: <100µs p99 latency
//! - HGET: <50µs p99 latency
//! - HGETALL (100 fields): <500µs p99 latency
//!
//! # Architecture
//! ```text
//! HashStore
//!   ├─ 64 shards (Arc<RwLock<HashMap<key, HashMap<field, value>>>>)
//!   └─ TTL applies to entire hash, not individual fields
//! ```

use super::error::{Result, SynapError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tracing::trace;

const SHARD_COUNT: usize = 64;

/// Hash value stored in a single key
/// Contains multiple field-value pairs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashValue {
    /// Field -> Value mapping
    pub fields: HashMap<String, Vec<u8>>,
    /// TTL for entire hash (applies to all fields)
    pub ttl_secs: Option<u64>,
    /// Creation timestamp
    pub created_at: u64,
    /// Last modification timestamp
    pub updated_at: u64,
}

impl HashValue {
    /// Create new hash value
    pub fn new(ttl_secs: Option<u64>) -> Self {
        let now = Self::current_timestamp();
        Self {
            fields: HashMap::new(),
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

    /// Check if hash has expired
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_secs {
            let now = Self::current_timestamp();
            now >= self.created_at + ttl
        } else {
            false
        }
    }

    /// Get number of fields
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Check if hash is empty
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Set a field value, returns true if field was created (false if updated)
    pub fn set_field(&mut self, field: String, value: Vec<u8>) -> bool {
        self.updated_at = Self::current_timestamp();
        self.fields.insert(field, value).is_none()
    }

    /// Get a field value
    pub fn get_field(&self, field: &str) -> Option<&Vec<u8>> {
        self.fields.get(field)
    }

    /// Delete field(s), returns number of fields deleted
    pub fn delete_fields(&mut self, fields: &[String]) -> usize {
        self.updated_at = Self::current_timestamp();
        fields.iter().filter_map(|f| self.fields.remove(f)).count()
    }

    /// Check if field exists
    pub fn has_field(&self, field: &str) -> bool {
        self.fields.contains_key(field)
    }

    /// Get all field names
    pub fn field_names(&self) -> Vec<String> {
        self.fields.keys().cloned().collect()
    }

    /// Get all values
    pub fn values(&self) -> Vec<Vec<u8>> {
        self.fields.values().cloned().collect()
    }

    /// Get all field-value pairs
    pub fn get_all(&self) -> HashMap<String, Vec<u8>> {
        self.fields.clone()
    }

    /// Set multiple fields atomically
    pub fn set_multiple(&mut self, fields: HashMap<String, Vec<u8>>) {
        self.updated_at = Self::current_timestamp();
        self.fields.extend(fields);
    }
}

/// Single shard of hash storage
struct HashShard {
    data: RwLock<HashMap<String, HashValue>>,
}

impl HashShard {
    fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

/// Hash store with 64-way sharding for lock-free concurrency
#[derive(Clone)]
pub struct HashStore {
    shards: Arc<[Arc<HashShard>; SHARD_COUNT]>,
    stats: Arc<RwLock<HashStats>>,
}

/// Statistics for hash operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HashStats {
    pub total_hashes: usize,
    pub total_fields: usize,
    pub hset_count: u64,
    pub hget_count: u64,
    pub hdel_count: u64,
    pub total_memory_bytes: usize,
}

impl HashStore {
    /// Create new hash store
    pub fn new() -> Self {
        // Initialize 64 shards
        let mut shards: Vec<Arc<HashShard>> = Vec::with_capacity(SHARD_COUNT);
        for _ in 0..SHARD_COUNT {
            shards.push(Arc::new(HashShard::new()));
        }

        let shards_array: [Arc<HashShard>; SHARD_COUNT] = shards
            .try_into()
            .unwrap_or_else(|_| panic!("Failed to convert Vec to array"));

        Self {
            shards: Arc::new(shards_array),
            stats: Arc::new(RwLock::new(HashStats::default())),
        }
    }

    /// Get shard index for a key using CRC32
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % SHARD_COUNT
    }

    /// Get reference to shard for a key
    fn shard_for_key(&self, key: &str) -> &Arc<HashShard> {
        &self.shards[self.shard_index(key)]
    }

    /// HSET - Set field value in hash
    /// Returns true if field was created, false if updated
    pub fn hset(&self, key: &str, field: &str, value: Vec<u8>) -> Result<bool> {
        let shard = self.shard_for_key(key);
        let mut data = shard.data.write();

        // Get or create hash
        let hash = data
            .entry(key.to_string())
            .or_insert_with(|| HashValue::new(None));

        // Check if expired
        if hash.is_expired() {
            data.remove(key);
            let new_hash = HashValue::new(None);
            data.insert(key.to_string(), new_hash);
            let hash = data.get_mut(key).unwrap();
            let created = hash.set_field(field.to_string(), value);

            // Update stats
            let mut stats = self.stats.write();
            stats.hset_count += 1;
            stats.total_fields += 1;

            return Ok(created);
        }

        let created = hash.set_field(field.to_string(), value);

        // Update stats
        let mut stats = self.stats.write();
        stats.hset_count += 1;
        if created {
            stats.total_fields += 1;
        }

        trace!("HSET key={} field={} created={}", key, field, created);
        Ok(created)
    }

    /// HGET - Get field value from hash
    pub fn hget(&self, key: &str, field: &str) -> Result<Option<Vec<u8>>> {
        let shard = self.shard_for_key(key);
        let data = shard.data.read();

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.hget_count += 1;
        }

        if let Some(hash) = data.get(key) {
            // Check if expired
            if hash.is_expired() {
                drop(data);
                // Remove expired hash
                let mut data_write = shard.data.write();
                data_write.remove(key);
                return Ok(None);
            }

            trace!(
                "HGET key={} field={} found={}",
                key,
                field,
                hash.has_field(field)
            );
            Ok(hash.get_field(field).cloned())
        } else {
            trace!("HGET key={} field={} not_found", key, field);
            Ok(None)
        }
    }

    /// HDEL - Delete field(s) from hash
    /// Returns number of fields deleted
    pub fn hdel(&self, key: &str, fields: &[String]) -> Result<usize> {
        let shard = self.shard_for_key(key);
        let mut data = shard.data.write();

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.hdel_count += 1;
        }

        if let Some(hash) = data.get_mut(key) {
            // Check if expired
            if hash.is_expired() {
                data.remove(key);
                return Ok(0);
            }

            let deleted = hash.delete_fields(fields);

            // Remove hash if empty
            if hash.is_empty() {
                data.remove(key);
            }

            // Update stats
            if deleted > 0 {
                let mut stats = self.stats.write();
                stats.total_fields = stats.total_fields.saturating_sub(deleted);
            }

            trace!("HDEL key={} fields={:?} deleted={}", key, fields, deleted);
            Ok(deleted)
        } else {
            trace!("HDEL key={} not_found", key);
            Ok(0)
        }
    }

    /// HEXISTS - Check if field exists in hash
    pub fn hexists(&self, key: &str, field: &str) -> Result<bool> {
        let shard = self.shard_for_key(key);
        let data = shard.data.read();

        if let Some(hash) = data.get(key) {
            if hash.is_expired() {
                drop(data);
                let mut data_write = shard.data.write();
                data_write.remove(key);
                return Ok(false);
            }

            Ok(hash.has_field(field))
        } else {
            Ok(false)
        }
    }

    /// HGETALL - Get all field-value pairs from hash
    pub fn hgetall(&self, key: &str) -> Result<HashMap<String, Vec<u8>>> {
        let shard = self.shard_for_key(key);
        let data = shard.data.read();

        if let Some(hash) = data.get(key) {
            if hash.is_expired() {
                drop(data);
                let mut data_write = shard.data.write();
                data_write.remove(key);
                return Ok(HashMap::new());
            }

            Ok(hash.get_all())
        } else {
            Ok(HashMap::new())
        }
    }

    /// HKEYS - Get all field names from hash
    pub fn hkeys(&self, key: &str) -> Result<Vec<String>> {
        let shard = self.shard_for_key(key);
        let data = shard.data.read();

        if let Some(hash) = data.get(key) {
            if hash.is_expired() {
                drop(data);
                let mut data_write = shard.data.write();
                data_write.remove(key);
                return Ok(Vec::new());
            }

            Ok(hash.field_names())
        } else {
            Ok(Vec::new())
        }
    }

    /// HVALS - Get all values from hash
    pub fn hvals(&self, key: &str) -> Result<Vec<Vec<u8>>> {
        let shard = self.shard_for_key(key);
        let data = shard.data.read();

        if let Some(hash) = data.get(key) {
            if hash.is_expired() {
                drop(data);
                let mut data_write = shard.data.write();
                data_write.remove(key);
                return Ok(Vec::new());
            }

            Ok(hash.values())
        } else {
            Ok(Vec::new())
        }
    }

    /// HLEN - Get number of fields in hash
    pub fn hlen(&self, key: &str) -> Result<usize> {
        let shard = self.shard_for_key(key);
        let data = shard.data.read();

        if let Some(hash) = data.get(key) {
            if hash.is_expired() {
                drop(data);
                let mut data_write = shard.data.write();
                data_write.remove(key);
                return Ok(0);
            }

            Ok(hash.len())
        } else {
            Ok(0)
        }
    }

    /// HMSET - Set multiple fields atomically
    pub fn hmset(&self, key: &str, fields: HashMap<String, Vec<u8>>) -> Result<()> {
        let shard = self.shard_for_key(key);
        let mut data = shard.data.write();

        // Get or create hash
        let hash = data
            .entry(key.to_string())
            .or_insert_with(|| HashValue::new(None));

        // Check if expired
        if hash.is_expired() {
            data.remove(key);
            let mut new_hash = HashValue::new(None);
            let field_count = fields.len();
            new_hash.set_multiple(fields);
            data.insert(key.to_string(), new_hash);

            // Update stats
            let mut stats = self.stats.write();
            stats.hset_count += field_count as u64;
            stats.total_fields += field_count;

            return Ok(());
        }

        let field_count = fields.len();
        hash.set_multiple(fields);

        // Update stats
        let mut stats = self.stats.write();
        stats.hset_count += field_count as u64;
        stats.total_fields += field_count;

        Ok(())
    }

    /// HMGET - Get multiple field values
    pub fn hmget(&self, key: &str, fields: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        let shard = self.shard_for_key(key);
        let data = shard.data.read();

        if let Some(hash) = data.get(key) {
            if hash.is_expired() {
                drop(data);
                let mut data_write = shard.data.write();
                data_write.remove(key);
                return Ok(vec![None; fields.len()]);
            }

            Ok(fields.iter().map(|f| hash.get_field(f).cloned()).collect())
        } else {
            Ok(vec![None; fields.len()])
        }
    }

    /// HINCRBY - Increment field value by integer
    pub fn hincrby(&self, key: &str, field: &str, increment: i64) -> Result<i64> {
        let shard = self.shard_for_key(key);
        let mut data = shard.data.write();

        // Get or create hash
        let hash = data
            .entry(key.to_string())
            .or_insert_with(|| HashValue::new(None));

        // Check if expired
        if hash.is_expired() {
            data.remove(key);
            let mut new_hash = HashValue::new(None);
            let new_value = increment;
            new_hash.set_field(field.to_string(), new_value.to_string().into_bytes());
            data.insert(key.to_string(), new_hash);

            // Update stats
            let mut stats = self.stats.write();
            stats.hset_count += 1;
            stats.total_fields += 1;

            return Ok(new_value);
        }

        // Get current value or default to 0
        let current_value = if let Some(bytes) = hash.get_field(field) {
            let s = String::from_utf8(bytes.clone()).map_err(|_| {
                SynapError::InvalidValue("Field value is not a valid integer".into())
            })?;
            s.parse::<i64>().map_err(|_| {
                SynapError::InvalidValue("Field value is not a valid integer".into())
            })?
        } else {
            0
        };

        let new_value = current_value
            .checked_add(increment)
            .ok_or_else(|| SynapError::InvalidValue("Integer overflow".into()))?;

        hash.set_field(field.to_string(), new_value.to_string().into_bytes());

        // Update stats
        let mut stats = self.stats.write();
        stats.hset_count += 1;

        Ok(new_value)
    }

    /// HINCRBYFLOAT - Increment field value by float
    pub fn hincrbyfloat(&self, key: &str, field: &str, increment: f64) -> Result<f64> {
        let shard = self.shard_for_key(key);
        let mut data = shard.data.write();

        // Get or create hash
        let hash = data
            .entry(key.to_string())
            .or_insert_with(|| HashValue::new(None));

        // Check if expired
        if hash.is_expired() {
            data.remove(key);
            let mut new_hash = HashValue::new(None);
            let new_value = increment;
            new_hash.set_field(field.to_string(), new_value.to_string().into_bytes());
            data.insert(key.to_string(), new_hash);

            // Update stats
            let mut stats = self.stats.write();
            stats.hset_count += 1;
            stats.total_fields += 1;

            return Ok(new_value);
        }

        // Get current value or default to 0.0
        let current_value = if let Some(bytes) = hash.get_field(field) {
            let s = String::from_utf8(bytes.clone())
                .map_err(|_| SynapError::InvalidValue("Field value is not a valid float".into()))?;
            s.parse::<f64>()
                .map_err(|_| SynapError::InvalidValue("Field value is not a valid float".into()))?
        } else {
            0.0
        };

        let new_value = current_value + increment;

        hash.set_field(field.to_string(), new_value.to_string().into_bytes());

        // Update stats
        let mut stats = self.stats.write();
        stats.hset_count += 1;

        Ok(new_value)
    }

    /// HSETNX - Set field value only if field does not exist
    /// Returns true if field was created, false if already exists
    pub fn hsetnx(&self, key: &str, field: &str, value: Vec<u8>) -> Result<bool> {
        let shard = self.shard_for_key(key);
        let mut data = shard.data.write();

        // Get or create hash
        let hash = data
            .entry(key.to_string())
            .or_insert_with(|| HashValue::new(None));

        // Check if expired
        if hash.is_expired() {
            data.remove(key);
            let mut new_hash = HashValue::new(None);
            new_hash.set_field(field.to_string(), value);
            data.insert(key.to_string(), new_hash);

            // Update stats
            let mut stats = self.stats.write();
            stats.hset_count += 1;
            stats.total_fields += 1;

            return Ok(true);
        }

        // Only set if field doesn't exist
        if hash.has_field(field) {
            Ok(false)
        } else {
            hash.set_field(field.to_string(), value);

            // Update stats
            let mut stats = self.stats.write();
            stats.hset_count += 1;
            stats.total_fields += 1;

            Ok(true)
        }
    }

    /// Get statistics
    pub fn stats(&self) -> HashStats {
        let stats = self.stats.read();
        stats.clone()
    }

    /// Clear all hashes (for testing)
    #[cfg(test)]
    pub fn clear(&self) {
        for shard in self.shards.iter() {
            shard.data.write().clear();
        }
        *self.stats.write() = HashStats::default();
    }
}

impl Default for HashStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hset_hget() {
        let store = HashStore::new();

        // Set field
        let created = store.hset("user:1000", "name", b"Alice".to_vec()).unwrap();
        assert!(created, "Field should be created");

        // Get field
        let value = store.hget("user:1000", "name").unwrap();
        assert_eq!(value, Some(b"Alice".to_vec()));

        // Update field
        let created = store.hset("user:1000", "name", b"Bob".to_vec()).unwrap();
        assert!(!created, "Field should be updated, not created");

        let value = store.hget("user:1000", "name").unwrap();
        assert_eq!(value, Some(b"Bob".to_vec()));
    }

    #[test]
    fn test_hget_nonexistent() {
        let store = HashStore::new();

        // Get from non-existent hash
        let value = store.hget("user:9999", "name").unwrap();
        assert_eq!(value, None);

        // Get non-existent field from existing hash
        store.hset("user:1000", "name", b"Alice".to_vec()).unwrap();
        let value = store.hget("user:1000", "age").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_hdel() {
        let store = HashStore::new();

        // Setup
        store.hset("user:1000", "name", b"Alice".to_vec()).unwrap();
        store.hset("user:1000", "age", b"30".to_vec()).unwrap();
        store
            .hset("user:1000", "email", b"alice@example.com".to_vec())
            .unwrap();

        // Delete single field
        let deleted = store.hdel("user:1000", &[String::from("email")]).unwrap();
        assert_eq!(deleted, 1);

        let value = store.hget("user:1000", "email").unwrap();
        assert_eq!(value, None);

        // Delete multiple fields
        let deleted = store
            .hdel("user:1000", &[String::from("name"), String::from("age")])
            .unwrap();
        assert_eq!(deleted, 2);

        // Hash should be removed when empty
        let len = store.hlen("user:1000").unwrap();
        assert_eq!(len, 0);
    }

    #[test]
    fn test_hexists() {
        let store = HashStore::new();

        store.hset("user:1000", "name", b"Alice".to_vec()).unwrap();

        assert!(store.hexists("user:1000", "name").unwrap());
        assert!(!store.hexists("user:1000", "age").unwrap());
        assert!(!store.hexists("user:9999", "name").unwrap());
    }

    #[test]
    fn test_hgetall() {
        let store = HashStore::new();

        store.hset("user:1000", "name", b"Alice".to_vec()).unwrap();
        store.hset("user:1000", "age", b"30".to_vec()).unwrap();
        store
            .hset("user:1000", "email", b"alice@example.com".to_vec())
            .unwrap();

        let all = store.hgetall("user:1000").unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all.get("name"), Some(&b"Alice".to_vec()));
        assert_eq!(all.get("age"), Some(&b"30".to_vec()));
        assert_eq!(all.get("email"), Some(&b"alice@example.com".to_vec()));

        // Non-existent hash returns empty map
        let all = store.hgetall("user:9999").unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn test_hkeys_hvals() {
        let store = HashStore::new();

        store.hset("user:1000", "name", b"Alice".to_vec()).unwrap();
        store.hset("user:1000", "age", b"30".to_vec()).unwrap();

        let keys = store.hkeys("user:1000").unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&String::from("name")));
        assert!(keys.contains(&String::from("age")));

        let vals = store.hvals("user:1000").unwrap();
        assert_eq!(vals.len(), 2);
        assert!(vals.contains(&b"Alice".to_vec()));
        assert!(vals.contains(&b"30".to_vec()));
    }

    #[test]
    fn test_hlen() {
        let store = HashStore::new();

        assert_eq!(store.hlen("user:1000").unwrap(), 0);

        store.hset("user:1000", "name", b"Alice".to_vec()).unwrap();
        assert_eq!(store.hlen("user:1000").unwrap(), 1);

        store.hset("user:1000", "age", b"30".to_vec()).unwrap();
        assert_eq!(store.hlen("user:1000").unwrap(), 2);

        store.hdel("user:1000", &[String::from("name")]).unwrap();
        assert_eq!(store.hlen("user:1000").unwrap(), 1);
    }

    #[test]
    fn test_hmset_hmget() {
        let store = HashStore::new();

        let mut fields = HashMap::new();
        fields.insert(String::from("name"), b"Alice".to_vec());
        fields.insert(String::from("age"), b"30".to_vec());
        fields.insert(String::from("email"), b"alice@example.com".to_vec());

        store.hmset("user:1000", fields).unwrap();

        let values = store
            .hmget(
                "user:1000",
                &[
                    String::from("name"),
                    String::from("age"),
                    String::from("nonexistent"),
                ],
            )
            .unwrap();

        assert_eq!(values.len(), 3);
        assert_eq!(values[0], Some(b"Alice".to_vec()));
        assert_eq!(values[1], Some(b"30".to_vec()));
        assert_eq!(values[2], None);
    }

    #[test]
    fn test_hincrby() {
        let store = HashStore::new();

        // Increment non-existent field (starts at 0)
        let result = store.hincrby("stats:user:1000", "login_count", 1).unwrap();
        assert_eq!(result, 1);

        // Increment existing field
        let result = store.hincrby("stats:user:1000", "login_count", 5).unwrap();
        assert_eq!(result, 6);

        // Decrement (negative increment)
        let result = store.hincrby("stats:user:1000", "login_count", -2).unwrap();
        assert_eq!(result, 4);
    }

    #[test]
    fn test_hincrbyfloat() {
        let store = HashStore::new();

        // Increment non-existent field (starts at 0.0)
        let result = store.hincrbyfloat("stats:user:1000", "score", 1.5).unwrap();
        assert!((result - 1.5).abs() < f64::EPSILON);

        // Increment existing field
        let result = store.hincrbyfloat("stats:user:1000", "score", 2.3).unwrap();
        assert!((result - 3.8).abs() < 0.001);

        // Decrement
        let result = store
            .hincrbyfloat("stats:user:1000", "score", -1.8)
            .unwrap();
        assert!((result - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_hsetnx() {
        let store = HashStore::new();

        // Set non-existent field
        let created = store
            .hsetnx("user:1000", "name", b"Alice".to_vec())
            .unwrap();
        assert!(created);

        // Try to set existing field
        let created = store.hsetnx("user:1000", "name", b"Bob".to_vec()).unwrap();
        assert!(!created);

        // Value should not have changed
        let value = store.hget("user:1000", "name").unwrap();
        assert_eq!(value, Some(b"Alice".to_vec()));
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(HashStore::new());
        let mut handles = vec![];

        // Spawn 100 threads doing concurrent HSET
        for i in 0..100 {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                for j in 0..10 {
                    let field = format!("field_{}", j);
                    let value = format!("value_{}_{}", i, j).into_bytes();
                    store_clone.hset("test:concurrent", &field, value).unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all fields exist
        let len = store.hlen("test:concurrent").unwrap();
        assert_eq!(len, 10); // Should have 10 unique fields
    }

    #[test]
    fn test_stats() {
        let store = HashStore::new();

        store.hset("user:1", "name", b"Alice".to_vec()).unwrap();
        store.hset("user:1", "age", b"30".to_vec()).unwrap();
        store.hget("user:1", "name").unwrap();

        let stats = store.stats();
        assert!(stats.hset_count >= 2);
        assert!(stats.hget_count >= 1);
        assert!(stats.total_fields >= 2);
    }
}
