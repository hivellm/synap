# Proposal: phase6g_v1-memory-accounting

Source: docs/analysis/synap-audit/ (M-018)

## Why
The memory budget that drives eviction only tracks the KV store, and reads copy full values.
`total_memory_bytes` is maintained and checked exclusively in `kv_store/store.rs` (54 hits),
with eviction running there; Hash/List/Set/SortedSet/Stream/Queue sizes are never added to the
budget. As a result `maxmemory` can be blown far past its limit by collections and brokers —
eviction only sheds KV strings while everything else grows unbounded, defeating the purpose of
a memory limit. Separately, every GET clones the entire value (`store.rs:454,492`
`value.data().to_vec()`), doubling memory traffic on large-value reads where Redis returns a
shared reference.

## What Changes
1. Extend memory accounting so Hash, List, Set, SortedSet, Stream and Queue allocations are
   counted toward the same `maxmemory` budget as KV, using per-datatype size estimators updated
   on mutation.
2. Make eviction consider the full accounted set (or, at minimum, refuse writes / apply the
   configured policy when the true total exceeds the limit) instead of only evicting KV.
3. Reduce copy overhead on the read path: store values behind a shared buffer type
   (`Arc<[u8]>`/`bytes::Bytes`) so GET can return without a full `to_vec()` clone.
4. Expose accurate per-datatype memory in INFO/metrics so the accounted total can be validated
   against RSS.

## Impact
- Affected specs: memory accounting + eviction coverage (MODIFIED)
- Affected code: `crates/synap-core/src/kv_store/store.rs`, `core/hash.rs`, `core/list.rs`,
  `core/set.rs`, `core/sorted_set.rs`, `core/stream.rs`, `core/queue.rs`, `monitoring/`
- Breaking change: NO in API; eviction now triggers correctly under real memory pressure
- User benefit: `maxmemory` is actually respected across all datatypes; large-value reads use
  less memory bandwidth
