//! Shared cross-datatype memory budget.
//!
//! A single [`GlobalMemory`] is shared by every store (KV, Hash, List, Set,
//! SortedSet, Stream, Queue) so the `maxmemory` limit accounts for all
//! datatypes, not just the KV store (audit M-018).
//!
//! Design: each store keeps its **own** `Arc<AtomicI64>` byte counter and
//! registers it once at construction. `GlobalMemory::used()` sums the registered
//! counters, so a store's existing per-mutation counter updates are reflected in
//! the shared total with no extra bookkeeping. On a growing write a store calls
//! [`GlobalMemory::would_exceed`] to refuse when the shared total is over the
//! cap; the KV eviction path uses [`GlobalMemory::used`] / [`GlobalMemory::max_bytes`].

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use parking_lot::RwLock;

/// Shared memory accounting across all datatypes, with an optional hard cap.
///
/// Cheap to clone (`Arc` inside). Counters are registered once at startup and
/// summed on demand; the registry read is an uncontended lock in the steady
/// state (no registrations after construction).
#[derive(Clone)]
pub struct GlobalMemory {
    counters: Arc<RwLock<Vec<Arc<AtomicI64>>>>,
    /// Hard cap in bytes; `0` means unlimited.
    max_bytes: i64,
}

impl GlobalMemory {
    /// Create a budget with a `max_bytes` cap (`0` = unlimited).
    pub fn new(max_bytes: usize) -> Self {
        Self {
            counters: Arc::new(RwLock::new(Vec::new())),
            max_bytes: max_bytes as i64,
        }
    }

    /// Register a store's byte counter so it contributes to the shared total.
    pub fn register(&self, counter: Arc<AtomicI64>) {
        self.counters.write().push(counter);
    }

    /// Current accounted usage in bytes — the sum of all registered counters.
    pub fn used(&self) -> i64 {
        self.counters
            .read()
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum()
    }

    /// Configured cap in bytes (`0` = unlimited).
    pub fn max_bytes(&self) -> i64 {
        self.max_bytes
    }

    /// True if adding `size` bytes would push the accounted total over the cap.
    /// Always false when the cap is unlimited or `size <= 0`.
    pub fn would_exceed(&self, size: i64) -> bool {
        self.max_bytes > 0 && size > 0 && self.used() + size > self.max_bytes
    }
}

impl std::fmt::Debug for GlobalMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalMemory")
            .field("used", &self.used())
            .field("max_bytes", &self.max_bytes)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_registered_counters_and_checks_cap() {
        let m = GlobalMemory::new(1000);
        let kv = Arc::new(AtomicI64::new(0));
        let hash = Arc::new(AtomicI64::new(0));
        m.register(kv.clone());
        m.register(hash.clone());
        assert_eq!(m.used(), 0);

        kv.fetch_add(600, Ordering::Relaxed);
        hash.fetch_add(300, Ordering::Relaxed);
        assert_eq!(m.used(), 900);

        // 900 + 200 > 1000 → would exceed.
        assert!(m.would_exceed(200));
        assert!(!m.would_exceed(100));

        // Freeing KV lets a collection write fit again.
        kv.fetch_sub(400, Ordering::Relaxed);
        assert_eq!(m.used(), 500);
        assert!(!m.would_exceed(200));
    }

    #[test]
    fn unlimited_never_exceeds() {
        let m = GlobalMemory::new(0);
        let c = Arc::new(AtomicI64::new(5_000_000_000));
        m.register(c);
        assert!(!m.would_exceed(1_000_000_000));
    }
}
