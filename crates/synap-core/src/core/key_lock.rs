//! Per-key lock manager for transaction isolation (audit M-010).
//!
//! A MULTI/EXEC must be isolated from non-transactional writers to the same
//! keys: a plain `SET k` issued while an `EXEC` touching `k` is running must be
//! ordered entirely before or after the transaction, never interleaved between
//! its commands. The EXEC serialization lock (phase6d) only orders transactions
//! against each other; this manager orders transactions against plain writers.
//!
//! Locks are **sharded** rather than one-mutex-per-key so the map never grows
//! unbounded: a key hashes to one of `SHARDS` async mutexes. Distinct keys can
//! share a shard (false contention — still correct, just serialized). Acquiring
//! multiple shards is always done in ascending shard-index order, which makes
//! deadlock between two concurrent multi-key acquisitions impossible.
//!
//! The mutexes are `tokio::sync::Mutex`, so a guard can be safely held across an
//! `.await` (unlike `parking_lot`), which EXEC needs while awaiting each store
//! op. Store write methods take the single-key lock; EXEC holds the whole key
//! set and calls the `*_unlocked` store methods to avoid re-entrant deadlock.

use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

/// Number of lock shards. A power of two keeps the modulo cheap and 1024 keeps
/// false contention low for realistic working sets while staying small in RAM.
const SHARDS: usize = 1024;

/// Sharded per-key async lock registry shared by [`KVStore`](crate::core::KVStore)
/// and the [`TransactionManager`](crate::core::TransactionManager).
pub struct KeyLockManager {
    shards: Vec<Arc<Mutex<()>>>,
}

impl KeyLockManager {
    /// Create a manager with a fixed number of lock shards.
    pub fn new() -> Self {
        Self {
            shards: (0..SHARDS).map(|_| Arc::new(Mutex::new(()))).collect(),
        }
    }

    /// FNV-1a hash → shard index. Deterministic and allocation-free.
    fn shard_index(key: &str) -> usize {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in key.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        (hash as usize) & (SHARDS - 1)
    }

    /// Lock the shard covering `key` for the lifetime of the returned guard.
    pub async fn lock_key(&self, key: &str) -> OwnedMutexGuard<()> {
        let idx = Self::shard_index(key);
        self.shards[idx].clone().lock_owned().await
    }

    /// Lock every distinct shard covering `keys`, in ascending shard order
    /// (deadlock-free), returning all guards. The caller holds them for the
    /// duration of a transaction so no plain writer to any of those keys can
    /// interleave.
    pub async fn lock_keys(&self, keys: &BTreeSet<String>) -> Vec<OwnedMutexGuard<()>> {
        let mut indices: Vec<usize> = keys.iter().map(|k| Self::shard_index(k)).collect();
        indices.sort_unstable();
        indices.dedup();

        let mut guards = Vec::with_capacity(indices.len());
        for idx in indices {
            guards.push(self.shards[idx].clone().lock_owned().await);
        }
        guards
    }
}

impl Default for KeyLockManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lock_key_serializes_same_key() {
        let mgr = Arc::new(KeyLockManager::new());
        let g = mgr.lock_key("k").await;

        // A second acquisition of the same key must not be grantable while held.
        let mgr2 = Arc::clone(&mgr);
        let handle = tokio::spawn(async move {
            let _g2 = mgr2.lock_key("k").await;
            42
        });

        // The task cannot complete while we hold the lock.
        tokio::task::yield_now().await;
        assert!(!handle.is_finished());

        drop(g);
        assert_eq!(handle.await.unwrap(), 42);
    }

    #[tokio::test]
    async fn lock_keys_acquires_all() {
        let mgr = KeyLockManager::new();
        let keys: BTreeSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let guards = mgr.lock_keys(&keys).await;
        // At most one guard per distinct shard; never more than the key count.
        assert!(!guards.is_empty());
        assert!(guards.len() <= keys.len());
    }
}
