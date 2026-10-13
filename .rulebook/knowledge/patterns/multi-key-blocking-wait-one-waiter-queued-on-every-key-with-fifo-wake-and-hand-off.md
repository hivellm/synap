# Multi-key blocking wait: one waiter queued on every key, FIFO wake and hand-off

**Category**: concurrency
**Tags**: blocking-pop, blpop, bzpopmin, tokio, notify, lost-wakeup, fairness, phase33

## Description

BLPOP/BZPOPMIN on several keys used one broadcast receiver per key and
awaited them in turn (`for rx in receivers { rx.recv().await }`), so a push to
the second key never woke the client until the first key was pushed too, and
the receivers were subscribed after the emptiness check (lost wake-up window).
What works: one waiter per blocked client (its own `tokio::sync::Notify` plus
a `woken` flag), queued at the back of a per-key FIFO for every key it waits
on, registered BEFORE the last emptiness check. A push makes data visible,
releases the shard lock, then wakes the oldest waiters of the key that are not
already woken (one per element). `Notify::notify_one` with a single listener
stores a permit, so a wake sent while the waiter is busy popping is not lost.
The waiter clears `woken` under the registry lock before each attempt, so a
push that skipped it as "already woken" is visible to that attempt. Whenever a
waiter leaves (pop, timeout, cancellation via Drop), it first removes itself
from every queue and then wakes the next waiter of each of its keys that still
holds data: this covers a waiter that popped an earlier key (argument order),
absorbed wake-ups for several keys into one pop, or died holding a wake-up.
Keep the pop in the waiter (it must run under its own persistence guard and
record the pop), keep a `waiting` counter so pushes skip the lock when nobody
blocks, and never take a shard lock while holding the registry lock.

Test it deterministically with a guarded pop whose lock is a test-held
`tokio::sync::Mutex`: hold it, push to several keys, release; the woken waiter
then sees all pushes at once (argument order, hand-off to a second waiter,
cancellation with `JoinHandle::abort` while it waits for the gate). Wait for
"blocked" with a test-only queue-length accessor instead of sleeping.

## Example

let has_data = |key: &str| self.has_elements(key);
if let Some(p) = self.try_pop_any(&keys, left, &mut lock).await { return Ok(p); }
let registration = self.blocked.register(&keys, &has_data); // queued on all keys
let wait = async {
    loop {
        if let Some(p) = self.try_pop_any(&keys, left, &mut lock).await { return p; }
        registration.woken().await;
    }
};
// push side, after dropping the shard lock:
self.blocked.wake(key, values_pushed);

## When to Use

Any "wait until one of several resources has data, then take it" API where the
taker must do the take itself (under its own lock or bookkeeping).

## When NOT to Use

Designs where the producer can hand the item to the waiter directly under one
lock (Redis serves blocked clients inside the push); there the producer pops
on the waiter's behalf and no hand-off is needed.
