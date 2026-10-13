//! Per-key write ordering and the snapshot barrier (phase23).
//!
//! A write that is persisted or replicated goes through two steps: apply it to
//! the store, then hand its record to the WAL writer and the replication log.
//! [`WriteOrder`] makes those two steps one critical section per key: a writer
//! locks the stripes of the keys it touches, applies, submits its record and
//! only then unlocks. For one key the WAL (and replication) therefore receive
//! records in the order the writes were applied in memory.
//!
//! The same stripes are the snapshot barrier: a snapshot locks all of them,
//! so while it records its WAL cutoff and copies the stores no write is
//! between "applied" and "submitted". Every write is then either in the
//! snapshot image (submitted before the cutoff) or after the cutoff, never
//! both and never neither.
//!
//! Stripes are `tokio::sync::Mutex`es, so a guard can be held across the
//! store calls, some of which are `async`. Multi-key writers and the snapshot
//! lock stripes in ascending index order, so no two of them can deadlock. The
//! stripes are always the outermost lock: they are taken before the EXEC lock,
//! the `KeyLockManager` guards and any store shard lock.

use tokio::sync::{Mutex, MutexGuard};

/// Number of stripes. A power of two keeps the index a mask; 1024 keeps false
/// sharing between unrelated keys rare while the snapshot still locks them all
/// in microseconds.
const STRIPES: usize = 1024;

/// Striped per-key ordering lock shared by every write path of one
/// [`PersistenceLayer`](super::PersistenceLayer).
pub struct WriteOrder {
    stripes: Box<[Mutex<()>]>,
}

/// Guards over the stripes of one write's keys (or of every stripe, for the
/// snapshot). Dropping it releases them.
pub struct StripeGuards<'a> {
    _guards: Vec<MutexGuard<'a, ()>>,
}

impl WriteOrder {
    /// A lock with every stripe free.
    pub fn new() -> Self {
        Self {
            stripes: (0..STRIPES).map(|_| Mutex::new(())).collect(),
        }
    }

    /// FNV-1a hash of `key`, masked to a stripe index.
    fn stripe_index(key: &str) -> usize {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in key.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        (hash as usize) & (STRIPES - 1)
    }

    /// Lock the stripes covering `keys`, in ascending order, each once.
    pub async fn lock<'k>(&self, keys: impl IntoIterator<Item = &'k str>) -> StripeGuards<'_> {
        let mut indices: Vec<usize> = keys.into_iter().map(Self::stripe_index).collect();
        indices.sort_unstable();
        indices.dedup();
        self.lock_indices(indices).await
    }

    /// Lock every stripe, in ascending order: no write can be between applying
    /// and submitting its record while the returned guards live.
    pub async fn lock_all(&self) -> StripeGuards<'_> {
        self.lock_indices((0..STRIPES).collect()).await
    }

    async fn lock_indices(&self, indices: Vec<usize>) -> StripeGuards<'_> {
        let mut guards = Vec::with_capacity(indices.len());
        for index in indices {
            let stripe = &self.stripes[index];
            // Uncontended fast path: no waiter queue, no waker.
            let guard = match stripe.try_lock() {
                Ok(guard) => guard,
                Err(_) => stripe.lock().await,
            };
            guards.push(guard);
        }
        StripeGuards { _guards: guards }
    }
}

impl Default for WriteOrder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    /// Two writers to one key exclude each other; writers to keys on
    /// different stripes do not.
    #[tokio::test]
    async fn same_key_excludes_other_keys_do_not() {
        let order = WriteOrder::new();
        let held = order.lock(["k"]).await;
        let other = (0..10_000)
            .map(|i| format!("o{i}"))
            .find(|k| WriteOrder::stripe_index(k) != WriteOrder::stripe_index("k"))
            .unwrap();
        // A different stripe is free.
        drop(order.lock([other.as_str()]).await);
        // The same key is not.
        assert!(
            tokio::time::timeout(Duration::from_millis(50), order.lock(["k"]))
                .await
                .is_err()
        );
        drop(held);
        drop(order.lock(["k"]).await);
    }

    /// A multi-key lock that names one stripe twice takes it once (no
    /// self-deadlock), and `lock_all` waits for writers to finish.
    #[tokio::test]
    async fn duplicate_stripes_and_lock_all() {
        let order = Arc::new(WriteOrder::new());
        let a = "a".to_owned();
        let twin = (0..100_000)
            .map(|i| format!("t{i}"))
            .find(|k| WriteOrder::stripe_index(k) == WriteOrder::stripe_index(&a))
            .unwrap();
        let held = order.lock([a.as_str(), twin.as_str(), a.as_str()]).await;
        let all = {
            let order = Arc::clone(&order);
            tokio::spawn(async move {
                drop(order.lock_all().await);
            })
        };
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(
            !all.is_finished(),
            "lock_all ran while a writer held a stripe"
        );
        drop(held);
        all.await.unwrap();
    }
}
