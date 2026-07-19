# Proposal: Reduce KV SET Path Allocations — Clone, Clock, String, Batch MSET

## Why

The KV SET hot path performs 6 redundant or avoidable allocations on every operation. These
allocations add latency and GC/allocator pressure that accumulates under high throughput:

- S-06: `value.clone()` is called unconditionally even when the cache layer is disabled. The
  clone is only needed for the cache; the kv_store itself takes ownership.
- S-07: The system clock is read 3 times per SET (once in the handler, once in StoredValue::new,
  once for the cache TTL). A single read should be passed through.
- S-10: `format!("kv:{}", req.key)` allocates a new String on every SET solely for permission
  checking — a hot path allocation for a string that is immediately discarded.
- S-11: `MultiTenant::scope_kv_key()` allocates even when Hub is disabled (99% of deployments).
- S-12: `key.to_string()` in `data.insert(key.to_string(), stored)` allocates when the shard
  already owns the insert — could take `key: String` from the caller.
- S-14: MSET calls `set()` in a loop, acquiring and releasing the shard lock N times instead
  of batching all keys for the same shard into a single critical section.

Combined, these add 4-6 heap allocations per SET that can be eliminated without changing
observable behavior. The expected result is ≥2x throughput on the `SET small_key small_value`
microbenchmark.

Source: docs/analysis/synap-vs-redis/ (set-deep-dive S-06, S-07, S-10, S-11, S-12, S-14; execution-plan Phase 1.5)

## What Changes

- MODIFIED: `StoredValue::new()` — accept pre-computed timestamp instead of reading clock internally
- MODIFIED: `kv_store.rs::set()` — take `key: String` (owned) instead of `key: &str` to avoid re-allocation
- MODIFIED: SET handler in `handlers.rs` — read clock once, pass timestamp to `kv_store.set()`
- MODIFIED: SET handler — skip `value.clone()` when cache is not configured
- MODIFIED: Permission key construction — use a stack-allocated or borrowed key format instead of `format!("kv:{}")`
- MODIFIED: `MultiTenant::scope_kv_key()` — short-circuit with zero allocation when Hub is disabled
- MODIFIED: `kv_store.rs::mset()` — group keys by shard, acquire each shard lock once for the entire batch

## Impact

- Affected specs: specs/kv/spec.md
- Affected code: synap-server/src/core/kv_store.rs, synap-server/src/core/types.rs, synap-server/src/server/handlers.rs, synap-server/src/hub/multi_tenant.rs
- Breaking change: NO
- User benefit: ≥2x throughput on small key-value SET operations; measurably lower allocator pressure under sustained load
