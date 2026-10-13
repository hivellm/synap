//! Waiter registry for blocking pops (BLPOP, BRPOP, BRPOPLPUSH, BZPOPMIN,
//! BZPOPMAX).
//!
//! A blocked client registers **one** waiter in the FIFO queue of **every**
//! key it waits on, so data arriving on any of its keys wakes it (Redis
//! semantics), instead of waiting on each key's notification in turn.
//!
//! ## Protocol
//!
//! Blocking pop (the store's side):
//!
//! 1. try to pop (fast path, nothing registered);
//! 2. [`BlockingKeys::register`] on all keys;
//! 3. loop: try to pop from the first non-empty key in argument order; on
//!    failure [`Registration::woken`] and retry;
//! 4. dropping the [`Registration`] (success, timeout or cancellation) leaves
//!    every queue and hands the wake-up on (see below).
//!
//! Push (any write that adds data to a key): make the data visible, release
//! the shard lock, then [`BlockingKeys::wake`] the key.
//!
//! ## Why no wake-up is lost
//!
//! - **Check, then wait.** The attempt in step 3 runs after registration. A
//!   push whose data the attempt missed reached the store after the attempt's
//!   shard critical section, so it saw the registration (the shard lock orders
//!   the two) and wakes a waiter of that key. Each waiter owns a
//!   [`Notify`] with a single listener, so a wake-up sent while the waiter is
//!   busy popping is stored as a permit and consumed by the next `woken()`.
//! - **Skipping woken waiters.** A push wakes the oldest waiters of its key
//!   that are not already woken. A waiter already woken has not resumed yet;
//!   it clears its flag (under the registry lock) *before* its next attempt,
//!   so that attempt sees the push it was skipped for.
//! - **Hand-off.** A woken waiter can end up not using the data it was woken
//!   for: it pops an earlier key instead (argument order), absorbed wake-ups
//!   of several keys into one pop, times out, or is cancelled. Leaving the
//!   queues therefore wakes the next waiter of every one of its keys that
//!   still holds data. Leaving happens before the check, so a push after the
//!   check no longer sees this waiter and wakes someone else itself.
//!
//! ## Fairness
//!
//! Waiters of one key are woken oldest first, and a woken waiter whose attempt
//! fails (another client took the element first) keeps its place. Two limits
//! remain, both because the waiter, not the pusher, performs the pop (it must
//! run under its own persistence ordering guard and record the pop itself):
//! a client that arrives with data already present pops immediately, so it can
//! take an element pushed for a woken waiter that has not run yet; and waiters
//! woken by one multi-element push pop in whatever order they get scheduled.
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::Notify;

/// One blocked client, queued on each of its keys.
#[derive(Default)]
struct Waiter {
    notify: Notify,
    /// A push chose this waiter and it has not resumed since. Read and written
    /// only while holding [`BlockingKeys::queues`].
    woken: AtomicBool,
}

/// Whether a key holds data a blocking pop could take right now.
pub(crate) type HasData<'a> = dyn Fn(&str) -> bool + Send + Sync + 'a;

/// Per-key FIFO queues of blocked clients.
#[derive(Default)]
pub(crate) struct BlockingKeys {
    queues: Mutex<HashMap<String, VecDeque<Arc<Waiter>>>>,
    /// Registered waiters. Pushes skip the queue lock while it is zero, so
    /// workloads without blocking pops pay one atomic load per push.
    waiting: AtomicUsize,
}

impl BlockingKeys {
    /// Create an empty registry.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Wake up to `count` waiters blocked on `key`, oldest first, skipping
    /// waiters already woken. Call after the pushed data is visible in the
    /// store and the shard lock is released.
    pub(crate) fn wake(&self, key: &str, count: usize) {
        if count == 0 || self.waiting.load(Ordering::SeqCst) == 0 {
            return;
        }
        let queues = self.queues.lock();
        let Some(queue) = queues.get(key) else {
            return;
        };
        let mut left = count;
        for waiter in queue {
            if left == 0 {
                break;
            }
            if !waiter.woken.swap(true, Ordering::Relaxed) {
                waiter.notify.notify_one();
                left -= 1;
            }
        }
    }

    /// Queue one waiter at the back of every key in `keys` (duplicates are
    /// queued once). `has_data` reports whether a key still holds data when
    /// the registration is dropped, to hand unused wake-ups on.
    pub(crate) fn register<'a>(
        &'a self,
        keys: &[String],
        has_data: &'a HasData<'a>,
    ) -> Registration<'a> {
        let mut unique: Vec<String> = Vec::with_capacity(keys.len());
        for key in keys {
            if !unique.contains(key) {
                unique.push(key.clone());
            }
        }
        let waiter = Arc::new(Waiter::default());
        {
            let mut queues = self.queues.lock();
            for key in &unique {
                queues
                    .entry(key.clone())
                    .or_default()
                    .push_back(Arc::clone(&waiter));
            }
            self.waiting.fetch_add(1, Ordering::SeqCst);
        }
        Registration {
            owner: self,
            waiter,
            keys: unique,
            has_data,
        }
    }

    /// Number of waiters currently queued on `key`.
    #[cfg(test)]
    pub(crate) fn queued(&self, key: &str) -> usize {
        self.queues.lock().get(key).map_or(0, VecDeque::len)
    }
}

/// A blocked client's place in the queues of its keys. Dropping it leaves
/// every queue and wakes the next waiter of each key that still holds data.
pub(crate) struct Registration<'a> {
    owner: &'a BlockingKeys,
    waiter: Arc<Waiter>,
    keys: Vec<String>,
    has_data: &'a HasData<'a>,
}

impl Registration<'_> {
    /// Wait until a push on one of the keys wakes this waiter (returns at once
    /// if one already did since the last call), then mark it as resumed so
    /// later pushes can choose it again.
    pub(crate) async fn woken(&self) {
        self.waiter.notify.notified().await;
        let _queues = self.owner.queues.lock();
        self.waiter.woken.store(false, Ordering::Relaxed);
    }
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        {
            let mut queues = self.owner.queues.lock();
            for key in &self.keys {
                if let Some(queue) = queues.get_mut(key) {
                    queue.retain(|w| !Arc::ptr_eq(w, &self.waiter));
                    if queue.is_empty() {
                        queues.remove(key);
                    }
                }
            }
            self.owner.waiting.fetch_sub(1, Ordering::SeqCst);
        }
        for key in &self.keys {
            if (self.has_data)(key) {
                self.owner.wake(key, 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn keys(names: &[&str]) -> Vec<String> {
        names.iter().map(|k| (*k).to_owned()).collect()
    }

    async fn is_woken(reg: &Registration<'_>) -> bool {
        tokio::time::timeout(Duration::from_millis(50), reg.woken())
            .await
            .is_ok()
    }

    #[tokio::test]
    async fn wake_on_any_key_reaches_a_multi_key_waiter() {
        let blocked = BlockingKeys::new();
        let none = |_: &str| false;
        let reg = blocked.register(&keys(&["a", "b"]), &none);
        blocked.wake("b", 1);
        assert!(is_woken(&reg).await, "a wake on the second key is seen");
    }

    #[tokio::test]
    async fn wake_is_stored_until_the_waiter_waits() {
        let blocked = BlockingKeys::new();
        let none = |_: &str| false;
        let reg = blocked.register(&keys(&["a"]), &none);
        blocked.wake("a", 1); // nobody is awaiting yet
        assert!(is_woken(&reg).await, "the permit survives until woken()");
        assert!(!is_woken(&reg).await, "and is consumed once");
    }

    #[tokio::test]
    async fn wakes_oldest_waiters_first_and_skips_woken_ones() {
        let blocked = BlockingKeys::new();
        let none = |_: &str| false;
        let first = blocked.register(&keys(&["q"]), &none);
        let second = blocked.register(&keys(&["q"]), &none);
        let third = blocked.register(&keys(&["q"]), &none);
        blocked.wake("q", 1);
        blocked.wake("q", 1); // first is woken and not resumed: skip it
        assert!(is_woken(&first).await);
        assert!(is_woken(&second).await);
        assert!(!is_woken(&third).await, "two pushes wake two waiters");
    }

    #[tokio::test]
    async fn dropping_hands_the_wake_up_to_the_next_waiter_with_data() {
        let blocked = BlockingKeys::new();
        let data = |key: &str| key == "q";
        let first = blocked.register(&keys(&["q", "empty"]), &data);
        let second = blocked.register(&keys(&["q"]), &data);
        let other = blocked.register(&keys(&["empty"]), &data);
        blocked.wake("q", 1);
        drop(first); // woken, never used it
        assert!(is_woken(&second).await, "q still holds data: next waiter");
        assert!(!is_woken(&other).await, "keys without data wake nobody");
    }

    #[tokio::test]
    async fn dropping_the_last_waiter_empties_the_registry() {
        let blocked = BlockingKeys::new();
        let none = |_: &str| false;
        let reg = blocked.register(&keys(&["a", "a", "b"]), &none);
        assert_eq!(blocked.queued("a"), 1, "duplicate keys are queued once");
        drop(reg);
        assert_eq!(blocked.queued("a"), 0);
        assert_eq!(blocked.queued("b"), 0);
        assert_eq!(blocked.waiting.load(Ordering::SeqCst), 0);
        assert!(blocked.queues.lock().is_empty());
    }
}
