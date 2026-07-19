# Proposal: Implement KV Eviction — Approximated LRU, Eviction Policies, Memory Pressure Handling

## Why

Synap currently cannot be used as a real cache. The `EvictionPolicy` enum declares `None`, `Lru`,
`Lfu`, and `Ttl` variants but the only implemented behavior is `None` — when `max_memory_mb` is
exceeded, `kv_store.rs:313-320` returns `SynapError::MemoryLimitExceeded` immediately. There is
no eviction loop, no approximated LRU, and no selection of candidates to remove.

This means production applications using Synap as a bounded cache receive hard errors instead
of graceful degradation. Every real cache use case — session storage, API response caching,
rate limiting state — requires eviction to function correctly under memory pressure.

Redis implements approximated LRU by sampling N random keys per shard and evicting the K with
the oldest `last_access` timestamp. This approach provides near-optimal hit rates without the
overhead of a true LRU linked list. Synap already tracks `last_access: u32` in `StoredValue::Expiring`
and has `last_access` write tracking in place — the sampling loop is the missing piece.

Source: docs/analysis/synap-vs-redis/ (findings F-003; execution-plan Phase 1.3)

## What Changes

- MODIFIED: `kv_store.rs::set()` — before returning `MemoryLimitExceeded`, invoke eviction loop until memory is freed, then insert
- ADDED: `evict_until_memory_available(needed_bytes)` method on `KVStore` — samples N random keys per shard, evicts K oldest by `last_access`
- ADDED: Eviction policy implementations: `allkeys-lru`, `volatile-lru`, `allkeys-random`, `volatile-random`, `volatile-ttl`, `noeviction`
- MODIFIED: `KVConfig` — add `eviction_policy: EvictionPolicy` and `eviction_sample_size: usize` (default 5, matching Redis)
- MODIFIED: `EvictionPolicy` enum variants renamed to match Redis naming convention

## Impact

- Affected specs: specs/kv/spec.md
- Affected code: synap-server/src/core/kv_store.rs, synap-server/src/core/types.rs
- Breaking change: NO (default eviction policy is `noeviction`, preserving current behavior)
- User benefit: Synap becomes usable as a real bounded cache; applications no longer receive hard errors under memory pressure; hit rate degrades gracefully
