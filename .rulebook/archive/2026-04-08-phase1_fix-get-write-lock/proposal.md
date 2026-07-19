# Proposal: Fix GET Write Lock — AtomicU32 for LRU last_access Update

## Why

The KV GET path acquires a full shard write lock solely to update the `last_access` timestamp
for LRU tracking. This is the most impactful single-line correctness issue in the codebase:
64-way sharding provides architectural parallelism for reads, but the write lock in the GET
path serializes all concurrent readers to the same shard — effectively reducing read concurrency
to 1 per shard regardless of thread count.

Evidence at `kv_store.rs:371`: `let mut data = shard.data.write()` inside the read path.
This is a well-known anti-pattern: Redis avoids it by being single-threaded; Synap's multi-
threaded architecture makes it particularly damaging.

The fix is a 4-byte change: replace `last_access: u32` in `StoredValue::Expiring` with
`last_access: AtomicU32`. GET can then hold a read lock, return the value, and update
`last_access` via `store(now, Relaxed)` without ever promoting to a write lock.

Source: docs/analysis/synap-vs-redis/ (findings F-004; execution-plan Phase 1.4)

## What Changes

- MODIFIED: `StoredValue::Expiring.last_access` — change type from `u32` to `AtomicU32`
- MODIFIED: `kv_store.rs::get()` — change `shard.data.write()` to `shard.data.read()`; update `last_access` via `store(now, Relaxed)`
- MODIFIED: Eviction sampling in `evict_candidates()` — read `last_access` via `load(Relaxed)` for comparison

## Impact

- Affected specs: specs/kv/spec.md
- Affected code: synap-server/src/core/types.rs, synap-server/src/core/kv_store.rs
- Breaking change: NO (internal implementation detail; no API change)
- User benefit: Read-only benchmark on a single shard scales linearly with reader threads up to ~16 cores; cache hit throughput improves proportionally to reader concurrency
