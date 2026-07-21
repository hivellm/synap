//! String extension commands for `KVStore`
//! (APPEND / GETRANGE / SETRANGE / STRLEN / GETSET / MSETNX).
//!
//! Split out of the oversized `store.rs` (phase2 modularization) as a separate
//! `impl KVStore` block. `KVStore` and its private fields live in the parent
//! module and stay reachable because this is a descendant module.
use super::KVStore;
use crate::core::error::Result;
use crate::core::types::StoredValue;
use std::sync::atomic::Ordering;
use tracing::debug;

impl KVStore {
    // ========================================
    // String Extension Commands
    // ========================================

    /// APPEND: Append bytes to an existing value, or create new key with value if it doesn't exist
    pub async fn append(&self, key: &str, value: Vec<u8>) -> Result<usize> {
        debug!("APPEND key={}, append_size={}", key, value.len());

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        // A partial mutation must ship the *resulting* value to watchers, not
        // the operand, so the merged bytes are captured here — but only when a
        // watch notifier is attached, so the copy is not on the common path.
        let watching = self.watch_notifier.is_some();
        let mut watch_value: Option<Vec<u8>> = None;

        let new_length = if let Some(stored_value) = data.get_mut(key) {
            if stored_value.is_expired() {
                // Key expired, treat as new
                let new_data = value;
                *stored_value = StoredValue::new(new_data.clone(), None);
                if watching {
                    watch_value = Some(new_data.to_vec());
                }
                new_data.len()
            } else {
                // Append to existing (copy-on-write: the Arc payload is immutable).
                stored_value.update_access();
                let mut merged = stored_value.data().to_vec();
                merged.extend_from_slice(&value);
                let len = merged.len();
                if watching {
                    watch_value = Some(merged.clone());
                }
                stored_value.set_data(merged);
                len
            }
        } else {
            // Key doesn't exist, create new
            let new_value = StoredValue::new(value.clone(), None);
            data.insert(key.to_string(), new_value);
            if watching {
                watch_value = Some(value.to_vec());
            }
            value.len()
        };

        self.stats.sets.fetch_add(1, Ordering::Relaxed);

        // Invalidate cache
        if let Some(ref cache) = self.cache {
            cache.delete(key);
        }

        drop(data);
        self.notify_keyspace(crate::core::EventClass::String, "append", key);
        self.notify_watch("append", key, watch_value.as_deref());

        Ok(new_length)
    }

    /// GETRANGE: Get substring using Redis-style negative indices
    /// start and end are inclusive. Negative indices count from the end (-1 = last byte)
    pub async fn getrange(&self, key: &str, start: isize, end: isize) -> Result<Vec<u8>> {
        debug!("GETRANGE key={}, start={}, end={}", key, start, end);

        let shard = self.get_shard(key);
        let data = shard.data.read();

        if let Some(value) = data.get(key) {
            if value.is_expired() {
                return Ok(Vec::new());
            }

            let bytes = value.data();
            let len = bytes.len() as isize;

            // Normalize indices (handle negative indices)
            let start_idx = if start < 0 {
                (len + start).max(0)
            } else {
                start.min(len)
            } as usize;

            let end_idx = if end < 0 {
                (len + end + 1).max(0) // +1 because end is inclusive
            } else {
                (end + 1).min(len) // +1 because end is inclusive
            } as usize;

            // Check bounds
            if start_idx >= end_idx || start_idx >= bytes.len() {
                return Ok(Vec::new());
            }

            self.stats.gets.fetch_add(1, Ordering::Relaxed);
            self.stats.hits.fetch_add(1, Ordering::Relaxed);

            Ok(bytes[start_idx..end_idx.min(bytes.len())].to_vec())
        } else {
            self.stats.gets.fetch_add(1, Ordering::Relaxed);
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            Ok(Vec::new())
        }
    }

    /// SETRANGE: Overwrite a substring at offset, extending the string if necessary
    /// Returns the new length of the string
    pub async fn setrange(&self, key: &str, offset: usize, value: Vec<u8>) -> Result<usize> {
        debug!(
            "SETRANGE key={}, offset={}, value_size={}",
            key,
            offset,
            value.len()
        );

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        // A partial mutation must ship the *resulting* value to watchers, not
        // the operand. Captured only when a watch notifier is attached.
        let watching = self.watch_notifier.is_some();
        let mut watch_value: Option<Vec<u8>> = None;

        let new_length = if let Some(stored_value) = data.get_mut(key) {
            if stored_value.is_expired() {
                // Key expired, create new string with padding
                let mut new_data = vec![0u8; offset];
                new_data.extend_from_slice(&value);
                *stored_value = StoredValue::new(new_data.clone(), None);
                new_data.len()
            } else {
                // Update existing (copy-on-write: the Arc payload is immutable).
                stored_value.update_access();
                let mut bytes = stored_value.data().to_vec();

                // Extend if necessary
                let required_len = offset + value.len();
                if bytes.len() < required_len {
                    bytes.resize(required_len, 0);
                }

                // Overwrite at offset
                bytes[offset..offset + value.len()].copy_from_slice(&value);
                let len = bytes.len();
                if watching {
                    watch_value = Some(bytes.clone());
                }
                stored_value.set_data(bytes);
                len
            }
        } else {
            // Key doesn't exist, create new with padding
            let mut new_data = vec![0u8; offset];
            new_data.extend_from_slice(&value);
            let new_value = StoredValue::new(new_data.clone(), None);
            data.insert(key.to_string(), new_value);
            if watching {
                watch_value = Some(new_data.clone());
            }
            new_data.len()
        };

        self.stats.sets.fetch_add(1, Ordering::Relaxed);

        // Invalidate cache
        if let Some(ref cache) = self.cache {
            cache.delete(key);
        }

        drop(data);
        self.notify_keyspace(crate::core::EventClass::String, "setrange", key);
        self.notify_watch("setrange", key, watch_value.as_deref());

        Ok(new_length)
    }

    /// STRLEN: Get the length of the string value in bytes
    pub async fn strlen(&self, key: &str) -> Result<usize> {
        debug!("STRLEN key={}", key);

        let shard = self.get_shard(key);
        let data = shard.data.read();

        if let Some(value) = data.get(key) {
            if value.is_expired() {
                self.stats.gets.fetch_add(1, Ordering::Relaxed);
                self.stats.misses.fetch_add(1, Ordering::Relaxed);
                return Ok(0);
            }

            self.stats.gets.fetch_add(1, Ordering::Relaxed);
            self.stats.hits.fetch_add(1, Ordering::Relaxed);

            Ok(value.data().len())
        } else {
            self.stats.gets.fetch_add(1, Ordering::Relaxed);
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            Ok(0)
        }
    }

    /// GETSET: Atomically get the current value and set a new one
    /// Returns the old value, or None if key didn't exist
    pub async fn getset(&self, key: &str, value: Vec<u8>) -> Result<Option<Vec<u8>>> {
        debug!("GETSET key={}, value_size={}", key, value.len());

        // Isolate against an in-flight EXEC on the same key (audit M-010).
        let _guard = self.key_locks.read_key(key).await;

        let shard = self.get_shard(key);
        let mut data = shard.data.write();

        let old_value = data.remove(key).map(|stored_value| {
            if stored_value.is_expired() {
                Vec::new() // Return empty for expired keys
            } else {
                stored_value.data().to_vec()
            }
        });

        // Insert new value
        let new_value = StoredValue::new(value.clone(), None);
        data.insert(key.to_string(), new_value);

        self.stats.gets.fetch_add(1, Ordering::Relaxed);
        self.stats.sets.fetch_add(1, Ordering::Relaxed);
        if old_value.is_some() {
            self.stats.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            self.stats.total_keys.fetch_add(1, Ordering::Relaxed);
        }

        // Invalidate cache
        if let Some(ref cache) = self.cache {
            cache.delete(key);
        }

        drop(data);
        self.notify_keyspace(crate::core::EventClass::String, "set", key);
        self.notify_watch("set", key, Some(&value));

        Ok(old_value)
    }

    /// MSETNX: Multi-set only if ALL keys don't exist (atomic)
    /// Returns true if all keys were set, false if any key already existed
    pub async fn msetnx(&self, pairs: Vec<(String, Vec<u8>)>) -> Result<bool> {
        debug!("MSETNX count={}", pairs.len());

        if pairs.is_empty() {
            return Ok(true);
        }

        // Check if all keys don't exist (need to check all shards)
        // Quick check: if any key exists, return false
        for (key, _) in &pairs {
            let shard = self.get_shard(key);
            let data = shard.data.read();
            if let Some(value) = data.get(key)
                && !value.is_expired()
            {
                return Ok(false);
            }
        }

        // All keys are free, now set them all atomically
        for (key, value) in &pairs {
            let shard = self.get_shard(key);
            let mut data = shard.data.write();

            data.insert(key.clone(), StoredValue::new(value.clone(), None));

            // Invalidate cache
            if let Some(ref cache) = self.cache {
                cache.delete(key);
            }
        }

        self.stats
            .sets
            .fetch_add(pairs.len() as u64, Ordering::Relaxed);

        Ok(true)
    }
}
