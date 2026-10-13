# Taking a second shard guard of the same store while holding one self-deadlocks on a hash collision

**Category**: concurrency
**Tags**: deadlock, parking_lot, sharding, hyperloglog, pfmerge, tokio, phase11

## Description

HyperLogLogStore::pfmerge held `dest_shard.write()` and then called `source_shard.read()` per source. When a source hashed to the destination's shard (1/64), the thread requested a read guard on a parking_lot RwLock it already held for writing; parking_lot locks are not reentrant, so it blocked forever. The parked thread is a tokio worker, and every later command touching that shard (including stats(), which walks all shards) parks another worker, until HTTP, RESP3 and SynapRPC all stop answering. There is no panic, and with `panic = "abort"` in release, a live process with dead listeners means a lock, not a crash. It shipped from 2025-10-30 to 1.3.1 because unit tests use fixed key names that never collide, and it only surfaced as an intermittent full-server hang in the TypeScript S2S suite (random keys) during 1.3.2 validation. Rules: in sharded stores, either snapshot the sources (drop each guard) before locking the destination, as SUNIONSTORE/ZUNIONSTORE/BITOP do, or handle the same-shard case explicitly and lock distinct shards in ascending index order, as SMOVE/RPOPLPUSH/key_lock do. Test it by searching for a key whose shard_index equals the destination's and running the call on a thread with recv_timeout, so a regression fails instead of hanging the run. clippy::await_holding_lock does not catch this: there is no await involved.

## Example

// Wrong: second guard on the same RwLock when shards collide
let mut dest_map = self.shard(dest).write();
for src in &sources {
    let src_map = self.shard(src).read(); // blocks forever if same shard
}

// Right: snapshot sources first, then lock the destination
let snapshots: Vec<_> = sources.iter()
    .filter_map(|k| self.shard(k).read().get(k).cloned())
    .collect();
let mut dest_map = self.shard(dest).write();

## When to Use

Any multi-key operation over a sharded map guarded by parking_lot/std locks.

## When NOT to Use

Stores guarded by one global lock, or reentrant locks (parking_lot::ReentrantMutex), where nesting is safe by construction.
