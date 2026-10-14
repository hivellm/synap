# Striped order lock doubles as the snapshot barrier, with a queue mark for the cutoff

**Category**: persistence
**Tags**: persistence, wal, snapshot, ordering, concurrency, expiry, phase23

## Description

When a write is "apply in memory, then queue its record", two problems share
one cause: nothing ties the two steps together. Same-key writers can queue in
the opposite order from memory (resulting-value records then replay to an old
value), and a snapshot that reads the log position and then dumps the stores
captures writes whose records land after its cutoff (applied twice on
recovery). One striped lock fixes both: writers lock their keys' stripes
(sorted, deduplicated) around apply + a synchronous enqueue, then release and
await durability; the snapshot locks every stripe, enqueues a MARK into the
log writer's FIFO and copies the stores, releases, and only then awaits the
mark, which resolves to the offset of the first record queued after it. That
offset is exact even though offsets are assigned later by the writer task,
because FIFO position is fixed at enqueue time. Keep the stripes the
outermost lock (before EXEC and per-key isolation locks), use async mutexes
because store calls await, give blocking pops a guarded variant that locks
only around each pop attempt, and skip the lock entirely when no persistence
layer exists. Verify ordering by checking EVERY record in the log (each
APPEND record one byte longer than the previous), not just the recovered
final value: the final-value check passed even with the lock removed, the
per-record check caught the inversion immediately.

## Example

let order = layer.order([key]).await;          // stripes, outermost
let written = kv.append_returning(key, v).await?;
order.commit(record(written)).await?;           // enqueue, release, then await fsync

// snapshot
let barrier = order.lock_all().await;
let mark = wal.mark()?;                         // synchronous enqueue
let image = SnapshotImage::capture(stores).await?;
drop(barrier);
let cutoff = mark.offset().await?;

## When to Use

Any write path that applies to memory and then logs/replicates, where the
log is replayed on top of periodic images or replicas apply records in order.

## When NOT to Use

Logs that record commands before applying them under one global lock, or
images taken with copy-on-write (fork) that can read the log position atomically.
