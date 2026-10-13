# Design: phase23_consistent-snapshots-and-expiry

Inputs: findings D-04, D-07 and the same-key part of D-08
(`docs/analysis/next-release-review-2026-10-12/`), the phase21 write-path
inventory (`persistence::durable` is the one mutation path every transport
uses) and the phase22 WAL header.

## 1. What is wrong today

- **Apply and record are two steps.** `durable::<command>` applies the write
  to the store (which takes and drops its shard lock), then calls
  `PersistenceLayer::log_operation`, which replicates and sends the record to
  the WAL writer task. Nothing orders the second step: two writers to one key
  can apply A then B in memory and reach the WAL channel B then A. Replaying
  resulting-value records (APPEND, INCR, SETRANGE as `KVSet`) then ends on A's
  value.
- **The WAL offset is assigned by the writer task**, not by the caller
  (`Writer::write_entry` does `current_offset.fetch_add`). `maybe_snapshot`
  reads `current_offset()` and then dumps the stores one by one. A write that
  was applied before its store was dumped but had not yet been written by the
  writer task gets an offset at or after the cutoff: it is in the image and in
  the replay, so LPUSH/HINCRBY apply twice (D-04). A write applied after its
  store was dumped but logged before the cutoff is lost.
- **Expiry is relative.** `KVStore::dump` drops it, recovery restores every
  snapshot key as persistent, WAL records hold the TTL in seconds from the
  moment of the write and replay ignores `WALEntry.timestamp`, so TTLs restart
  at boot and keys that expired during downtime come back (D-07).

## 2. Ordering and the snapshot barrier: one striped lock (items 1.2, 1.3)

`persistence::order::WriteOrder` is an array of 1024 `tokio::sync::Mutex<()>`
stripes owned by the `PersistenceLayer`. A key maps to a stripe by FNV-1a
(the same function `KeyLockManager` uses).

**Writers.** Every `durable` function, when a layer exists, does

```text
guard = layer.order([keys])          lock the keys' stripes, ascending, deduplicated
effect = store.<command>(...)        apply in memory (store takes its own shard lock inside)
layer.submit(record)                 replicate + push onto the WAL channel  (synchronous)
drop(guard)
ack.await                            wait for the group commit / fsync outside the lock
```

The record enters the WAL channel (and the replication log) while the stripe
is held, so for one key the channel order is the memory order. The WAL writer
assigns offsets in channel order, so WAL order == memory order (1.3), and so
does replication order. Durability waiting stays outside the lock, so group
commit still batches concurrent writers.

**Snapshot.** The snapshot locks all 1024 stripes in ascending order. While
it holds them no writer is between "apply" and "submit":

1. it sends a *mark* request down the WAL channel (synchronous);
2. it captures an in-memory image of every WAL-backed store (KV with
   absolute expiry, queues, hashes, lists, sets, sorted sets, HyperLogLog,
   bitmaps);
3. it releases the stripes;
4. it awaits the mark: the writer answers with its `current_offset` at the
   mark's position in the channel, i.e. the offset of the first record sent
   after the barrier. That is the snapshot's WAL cutoff;
5. it captures streams (not in the WAL, owned by `StreamPersistence`),
   serializes and fsyncs the file — outside the barrier.

Every write either submitted before the barrier (its effect is in the image,
its record is before the mark, offset < cutoff, not replayed) or after it
(not in the image, offset >= cutoff, replayed). Never both, never neither.
Multi-key commands (MSET, SMOVE, RENAME, ZUNIONSTORE, ...) lock every key's
stripe, so they are on one side of the barrier as a whole; the store-by-store
dump is no longer taken "at mixed times" because no write is in flight while
the image is captured.

**Transactions.** `durable::exec` takes the transaction out of the
`TransactionManager` (`take_transaction`), locks the stripes of its key set,
runs it (`exec_transaction`, which still takes the EXEC lock and the
`KeyLockManager` write guards inside), submits the one `Transaction` record
and releases. The lock order everywhere is
`WriteOrder stripes -> EXEC lock -> KeyLockManager -> store shard locks`,
so neither plain writers nor EXEC can deadlock with each other or with the
snapshot (which takes only stripes, in ascending order, like every
multi-key writer).

**Blocking pops** (BLPOP, BRPOP, BRPOPLPUSH, BZPOPMIN, BZPOPMAX) must not
wait while holding a stripe (the pusher needs it). The list and sorted-set
stores gain `*_guarded` variants that take an async `lock` function: every
pop attempt runs inside the guard it returns, and a successful attempt hands
the guard back with the popped value so `durable` records the pop before
releasing it. Waiting for a push happens without the guard. The plain
`blpop`/`bzpopmin`/... call the guarded variant with a no-op lock, so stats,
wake-up and timeout logic stay in one place.

**Why stripes and not a global `RwLock` gate plus a per-key lock.** One
lock acquisition per write instead of two, and writers to different keys
touch different cache lines (a global read lock bounces one counter between
every core). The snapshot pays 1024 uncontended acquisitions (~tens of
microseconds) instead of one.

**Cost.**

- Persistence and replication off (`AppState.persistence == None`): no
  change. Every `durable` function still returns the plain store call before
  touching the layer; no lock, no allocation.
- Layer present, no contention: one `try_lock` on a stripe (an atomic CAS on
  a semaphore) and one small `Vec` of stripe guards: 69 ns per lock + unlock
  (release build, 1M iterations). The record was already allocated and sent
  before; the WAL round trip (channel + oneshot + group commit) is tens of
  microseconds and dominates. Durable SET, 8 tasks x 20,000 writes, Periodic
  fsync, release build, one run each: distinct keys 76k ops/s with the
  stripes vs 54k without, one hot key 53k vs 56k — differences inside the
  run-to-run noise of a WAL-bound workload.
- Same key, contended: writers to one key now serialize over
  apply + submit (a few hundred nanoseconds: the store call plus a channel
  push), not over the WAL wait. That is the price of 1.3; a hot single key was
  already serialized by its shard's data lock during apply.
- Snapshot: writers stall while the image is captured. The capture is an
  in-memory copy of the WAL-backed stores — KV values are shared `Arc<[u8]>`
  buffers (a refcount bump per key plus the key string), collections are
  cloned — with no I/O and no serialization: 58 ms for 160,000 KV keys plus
  10,000 ten-field hashes (release build), against 0.9 s for the whole
  snapshot. Serialization, stream capture, fsync and file rotation happen
  after the barrier is released.

## 3. Absolute expiry (items 1.4, 1.5, 1.6)

**Encoding.** Absolute expiry is Unix milliseconds, the unit `StoredValue`
already keeps (`expires_at`).

- New WAL / replication `Operation` variants, appended after `BitmapField`
  so every earlier variant index (1.3.x, phase21, phase22 records) still
  decodes:
  - `KVSetAt { key, value, expires_at_ms: Option<u64> }` — SET family with a
    deadline (SET EX/PX/EXAT/PXAT/KEEPTTL, INCR/APPEND/SETRANGE on an
    expiring key, transactions);
  - `KVExpireAt { key, expires_at_ms: u64 }` — EXPIRE;
  - `HllAddAt { key, elements, expires_at_ms: u64 }` — PFADD with a TTL.
  A write that leaves no expiry keeps the old record (`KVSet { ttl: None }`,
  `KVExpire { ttl: None }` for PERSIST, `HllAdd { ttl: None }`): "no expiry"
  is already absolute.
- The deadline recorded is the one the store stored, read under the same
  lock (`KVStore::set_returning_deadline`, `expire_returning_deadline`,
  `incr_with_expiry`, `append_returning`, `HyperLogLogStore::pfadd_with_expiry`),
  never recomputed from "now" in the caller.
- `CommittedWrite::KvSet` carries `expires_at_ms` instead of a relative TTL;
  `set_unlocked` returns the deadline it stored.

**Applying.** `apply_operation` (WAL replay and replicas) applies a deadline
as is (`Expiry::UnixMilliseconds`). A deadline that has already passed
deletes the key instead (the write happened, then the key expired), so
recovery and replicas never resurrect it.

**Legacy records (1.5).** Before applying a WAL entry, recovery converts the
relative-TTL forms to absolute ones using the entry's own timestamp:
`KVSet/KVExpire/HllAdd { ttl: Some(t) }` at `WALEntry.timestamp = T` becomes
the `*At` form with `(T + t) * 1000`, inside transaction records too. A key
whose deadline is already past is therefore omitted. (Entry timestamps are in
seconds, so legacy deadlines have one-second precision.)

**Snapshot v4.** Magic `SYNAP004`, version 4. The KV section adds an
`expires_at_ms: u64` after each value (`0` = no expiry; 0 is never a future
deadline). Two map sections follow the sorted-set section: HyperLogLog
(`bincode(HyperLogLogValue)`) and bitmaps (`bincode(BitmapValue)`); both
values already hold `created_at + ttl_secs`, i.e. an absolute deadline in
seconds. v3 and v2 files still load (KV without expiry, no HLL/bitmap
sections). Recovery skips snapshot keys whose deadline has passed and
restores the rest with their absolute deadline.

**Replication.** Streamed records use the same `Operation` values, so they
carry the `*At` forms. The full-sync image (`replication::sync`) now emits
`KVSetAt` with each key's absolute deadline (it used to emit the remaining
whole seconds), and the replica's full-sync applier accepts it.

**Downgrade.** A 1.3.x server cannot read a v4 snapshot (unknown magic) nor
a phase22 WAL (header). Downgrading needs a 1.3.x-compatible export; this is
documented in the persistence docs.

## 4. Out of scope (recorded, not silently dropped)

- The replication full-sync image is still KV + streams only and is taken
  without the barrier (the master's replication log is fed inside the same
  critical section, but `send_full_sync` reads its offset before dumping).
  Making full sync a consistent multi-datatype image is a replication change
  of its own.
- Multi-key BLPOP/BRPOP/BZPOP* wait for a notification on every key in turn
  (`for rx in receivers { rx.recv().await }`), so a push to the second key
  does not wake a waiter until the first key is pushed too. The guarded
  variants keep that behavior unchanged.
- TTL eviction and `maxmemory` eviction are not logged (unchanged); absolute
  expiry makes the former consistent on recovery.
