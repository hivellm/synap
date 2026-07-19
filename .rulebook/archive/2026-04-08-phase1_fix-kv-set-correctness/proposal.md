# Proposal: Fix KV SET Correctness — Memory Accounting, Atomic Stats, WAL Durability

## Why

The Synap KV SET hot path has 5 critical correctness bugs that make it unsuitable for production
cache workloads. These issues were discovered in real project deployments and are the #1 reported
pain point by users.

Memory accounting drifts on every overwrite (S-05): `total_memory_bytes` is incremented on every
insert but never decremented when an old value is replaced. After 10M SET-overwrite cycles the
reported memory diverges significantly from actual usage, breaking autoscaling and the
`max_memory_mb` guard.

Global stats write lock serializes every SET (S-04): `Arc<RwLock<KVStats>>` is acquired for write
on every SET, GET, DEL, and EXPIRE. With 64 shards providing shard-level parallelism, the global
stats lock negates the architectural advantage entirely.

WAL is written AFTER the memory write (S-08): The handler writes to memory, returns success to
the client, and then attempts WAL logging. If the WAL write fails, the data is in memory but not
durable — a silent durability lie. True WAL requires write-ahead semantics.

No max value size guard (S-13): A single large value can consume the entire memory budget.

INCR/DECR destroy TTL (S-16): Increment handlers replace Expiring entries with Persistent ones,
silently dropping the TTL on every numeric increment.

Source: docs/analysis/synap-vs-redis/ (findings F-006, F-012; set-deep-dive S-04, S-05, S-08, S-13, S-16)

## What Changes

- ADDED: `max_value_size_bytes` field to `KVConfig` in `core/types.rs`
- MODIFIED: `kv_store.rs::set()` — subtract old value size on overwrite (insert returns Some(old))
- MODIFIED: `kv_store.rs::delete()` and `cleanup_expired()` — decrement `total_memory_bytes`
- MODIFIED: `KVStats` — replace `Arc<RwLock<KVStats>>` with atomic counters (`AtomicU64`, `AtomicI64`)
- MODIFIED: All stat-update sites — use `fetch_add(_, Relaxed)` instead of write-lock
- MODIFIED: `handlers.rs` SET handler — validate value size before allocation; log WAL BEFORE responding
- MODIFIED: WAL config — add `durability` mode: `Sync` (fsync before respond) vs `Async` (current default)
- MODIFIED: `handlers.rs` INCR/DECR — preserve TTL when updating; use `checked_add` for overflow safety

## Impact

- Affected specs: specs/kv/spec.md
- Affected code: synap-server/src/core/kv_store.rs, synap-server/src/core/types.rs, synap-server/src/server/handlers.rs, synap-server/src/persistence/types.rs
- Breaking change: NO
- User benefit: Memory stats become accurate; concurrent SET throughput improves measurably; durability is honest and configurable; INCR preserves TTL; oversized values rejected before allocation
