# Proposal: phase13_write-scalability-under-contention

Source: phase13_multikey-benchmark-sweep findings (docs/benchmarks/redis-vs-synap.md, "Multi-key" section)

## Why

The multi-key sweep (`redis-benchmark -r 1000000 -P 16`) proved Synap beats
Redis 7 on SET/INCR/LPUSH (1.14–1.25×) at c=50 — but exposed two write-path
scalability defects:

1. **SADD multi-key regression: 0.57 of Redis** (392k rps vs 802k single-key).
   With 1M random keys nearly every op takes the miss path. Suspects, in order:
   `SetStore`'s `HashMap<String, SetValue>` and `SetValue`'s `HashSet<Vec<u8>>`
   both use the **default SipHash hasher** (KVStore uses `ahash`); every
   `SetValue::new`/`add` calls `SystemTime::now()` for timestamps (2 syscalls
   per op); per-miss allocations (HashSet + key String + SetValue struct).

2. **Writes collapse at c=200 while reads scale.** `-r 1000000, c=200, P=16`:
   GET 731k (**1.16× Redis** — wins), but SET **153k (0.24)** / INCR **112k
   (0.26)** — down from ~790k at c=50. ~3,200 in-flight ops ÷ 64 data shards ≈
   50 concurrent writers per shard; a contended **`parking_lot` write lock
   parks OS threads** — i.e. tokio worker threads — starving the whole runtime.
   Reads share the lock and don't collapse. Candidate fixes:
   - `try_read_owned()` fast path on the per-key `RwLock` (skip the async
     acquisition entirely when uncontended);
   - more/finer KV data shards (64 → 256+) to cut writers-per-shard;
   - bounding in-flight dispatch per connection (the SynapRPC/RESP3 loop spawns
     a task per request — 3,200 concurrent store calls is self-inflicted
     contention; Redis processes serially per connection);
   - profiling to confirm where the parked time actually goes before choosing.

## What Changes

Profile first (`tokio-console`/`perf` or targeted counters), then apply the
smallest fix per defect and re-run the multi-key sweep at c=50 and c=200:
- SADD target: ≥ 0.9 of Redis multi-key (from 0.57).
- SET/INCR at c=200 target: no collapse (≥ 0.8 of their c=50 numbers).
Each change keeps the full test suite green; results recorded in the benchmark
doc.

## Impact

- Affected specs: none
- Affected code: crates/synap-core/src/core/{set.rs,kv_store/*,key_lock.rs},
  possibly the protocol servers' per-request spawning
- Breaking change: NO
- User benefit: write throughput that scales with connections — the main
  real-world load shape — instead of collapsing past ~50 clients
