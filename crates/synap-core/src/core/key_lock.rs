//! Per-key lock manager for transaction isolation (audit M-010).
//!
//! A MULTI/EXEC must be isolated from non-transactional writers to the same
//! keys: a plain `SET k` issued while an `EXEC` touching `k` is running must be
//! ordered entirely before or after the transaction, never interleaved between
//! its commands. The EXEC serialization lock (phase6d) only orders transactions
//! against each other; this manager orders transactions against plain writers.
//!
//! ## Reader/writer split (phase12 write-lock fast-path)
//!
//! The isolation this manager provides is one-directional: plain writers must be
//! excluded **by** an EXEC, but plain writers do **not** need to exclude each
//! other — two `SET k` calls are already serialized by the KV shard's data lock,
//! which is the only thing that actually mutates state. So the primitive is a
//! **`RwLock`**, not a mutex:
//!
//! - plain writers take the **read** side (shared) — many concurrent writes to
//!   the same hot key no longer serialize on this lock;
//! - EXEC takes the **write** side (exclusive) over every touched shard — so no
//!   plain writer can interleave with the transaction's command sequence.
//!
//! Before phase12 this was a `tokio::Mutex` per shard, which serialized every
//! write to a key on a single async mutex; under pipelined single-hot-key load
//! that capped SET/INCR throughput. The read/write split keeps M-010 isolation
//! (EXEC still excludes plain writers) while letting plain writers proceed in
//! parallel.
//!
//! Locks are **sharded** rather than one-lock-per-key so the map never grows
//! unbounded: a key hashes to one of `SHARDS` async `RwLock`s. Distinct keys can
//! share a shard (false contention — still correct, just serialized). EXEC
//! acquires multiple shards in ascending shard-index order, which makes deadlock
//! between two concurrent multi-key acquisitions impossible.
//!
//! The locks are `tokio::sync::RwLock`, so a guard can be safely held across an
//! `.await` (unlike `parking_lot`), which EXEC needs while awaiting each store
//! op. Store write methods take the read guard; EXEC holds the write guards for
//! the whole key set and calls the `*_unlocked` store methods to avoid re-entrant
//! deadlock.

use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

/// Number of lock shards. A power of two keeps the modulo cheap and 1024 keeps
/// false contention low for realistic working sets while staying small in RAM.
const SHARDS: usize = 1024;

/// Sharded per-key async lock registry shared by [`KVStore`](crate::core::KVStore)
/// and the [`TransactionManager`](crate::core::TransactionManager).
pub struct KeyLockManager {
    shards: Vec<Arc<RwLock<()>>>,
}

impl KeyLockManager {
    /// Create a manager with a fixed number of lock shards.
    pub fn new() -> Self {
        Self {
            shards: (0..SHARDS).map(|_| Arc::new(RwLock::new(()))).collect(),
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

    /// Acquire the **read** (shared) side of the shard covering `key` — the plain
    /// non-transactional writer path. Concurrent plain writers to the same key
    /// share this guard and do not serialize on it; an in-flight EXEC (write
    /// side) still excludes them.
    pub async fn read_key(&self, key: &str) -> OwnedRwLockReadGuard<()> {
        let idx = Self::shard_index(key);
        // Fast path: no EXEC holds (or waits on) this shard — the common case.
        // `try_read_owned` grants the guard without touching tokio's async
        // acquire machinery (queue + waker), which matters under thousands of
        // in-flight pipelined writes (phase13 write-scalability).
        match self.shards[idx].clone().try_read_owned() {
            Ok(guard) => guard,
            Err(_) => self.shards[idx].clone().read_owned().await,
        }
    }

    /// Acquire the **write** (exclusive) side of every distinct shard covering
    /// `keys`, in ascending shard order (deadlock-free), returning all guards.
    /// EXEC holds them for the whole transaction so no plain writer to any of
    /// those keys can interleave (audit M-010).
    pub async fn write_keys(&self, keys: &BTreeSet<String>) -> Vec<OwnedRwLockWriteGuard<()>> {
        let mut indices: Vec<usize> = keys.iter().map(|k| Self::shard_index(k)).collect();
        indices.sort_unstable();
        indices.dedup();

        let mut guards = Vec::with_capacity(indices.len());
        for idx in indices {
            guards.push(self.shards[idx].clone().write_owned().await);
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
    async fn read_key_does_not_serialize_same_key() {
        // The fast-path win: two plain writers to the same key hold the read side
        // concurrently — neither blocks the other.
        let mgr = Arc::new(KeyLockManager::new());
        let g1 = mgr.read_key("k").await;

        let mgr2 = Arc::clone(&mgr);
        let handle = tokio::spawn(async move {
            let _g2 = mgr2.read_key("k").await; // must be grantable while g1 is held
            42
        });

        assert_eq!(handle.await.unwrap(), 42);
        drop(g1);
    }

    #[tokio::test]
    async fn write_excludes_reader_same_shard() {
        // M-010 isolation: while EXEC holds the write side, a plain writer (read)
        // on the same key cannot proceed until EXEC releases.
        let mgr = Arc::new(KeyLockManager::new());
        let keys: BTreeSet<String> = ["k"].iter().map(|s| s.to_string()).collect();
        let wguards = mgr.write_keys(&keys).await;

        let mgr2 = Arc::clone(&mgr);
        let handle = tokio::spawn(async move {
            let _r = mgr2.read_key("k").await;
            7
        });

        // The reader cannot complete while the write guard is held.
        tokio::task::yield_now().await;
        assert!(!handle.is_finished());

        drop(wguards);
        assert_eq!(handle.await.unwrap(), 7);
    }

    #[tokio::test]
    async fn reader_excludes_writer_same_shard() {
        // The other direction: a plain writer (read) in flight delays an EXEC
        // (write) on the same key until the writer releases — so the EXEC is
        // ordered entirely after it, never interleaved.
        let mgr = Arc::new(KeyLockManager::new());
        let r = mgr.read_key("k").await;

        let mgr2 = Arc::clone(&mgr);
        let handle = tokio::spawn(async move {
            let keys: BTreeSet<String> = ["k"].iter().map(|s| s.to_string()).collect();
            let _w = mgr2.write_keys(&keys).await;
            9
        });

        tokio::task::yield_now().await;
        assert!(!handle.is_finished());

        drop(r);
        assert_eq!(handle.await.unwrap(), 9);
    }

    #[tokio::test]
    async fn write_keys_acquires_all() {
        let mgr = KeyLockManager::new();
        let keys: BTreeSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let guards = mgr.write_keys(&keys).await;
        // At most one guard per distinct shard; never more than the key count.
        assert!(!guards.is_empty());
        assert!(guards.len() <= keys.len());
    }
}
