# Offset assigned outside the lock that orders the queue lets offset-deduplicating consumers drop records

**Category**: concurrency
**Tags**: replication, ordering, offset, concurrency, phase31

## Description

MasterNode::replicate took the replication offset with fetch_add, then pushed the record into the log buffer under a lock, then sent it to the fan-out channel. Two writers to different keys (different write-order stripes, so not serialized) could take offsets 5 and 6 and enqueue 6 first. The replica skips any record whose offset is below the one it expects (that is how it deduplicates records a full-sync image already covers), so it applied 6, expected 7, and silently dropped 5. Per-key ordering locks do not help: the inversion is between unrelated keys. Fix: assign the offset and enqueue for every downstream consumer inside ONE critical section (ReplicationLog::append_with runs the enqueue closure under the buffer lock). Verify with a concurrent test that records the enqueue order (strictly consecutive offsets), and with an end-to-end test of many writers on distinct keys while a replica follows: with the old two-step code plus a thread::yield_now between the steps, the end-to-end test failed 3/3 runs.

## Example

// Wrong: offset and enqueue are two steps
let offset = log.current_offset.fetch_add(1, SeqCst);
log.buffer.write().push_back(record(offset, op.clone()));
tx.send((offset, op));

// Right: one critical section orders offset, buffer and channel
log.append_with(op, |record| { let _ = tx.send(record.clone()); });

## When to Use

Any log/sequence where offsets are assigned by one step and records are queued to consumers by another, and consumers deduplicate or detect gaps by offset.

## When NOT to Use

Single-producer logs, or consumers that reorder by offset (a reorder buffer) before applying.
