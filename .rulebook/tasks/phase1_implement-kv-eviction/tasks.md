## 1. Eviction Policy Config
- [x] 1.1 Rename `EvictionPolicy` variants to Redis-compatible names: `NoEviction`, `AllKeysLru`, `VolatileLru`, `AllKeysRandom`, `VolatileRandom`, `VolatileTtl`
- [x] 1.2 Add `eviction_policy: EvictionPolicy` and `eviction_sample_size: usize` (default 5) to `KVConfig`

## 2. Approximated LRU Sampling Loop
- [x] 2.1 Implement `KVStore::evict_candidates(shard, sample_size) -> Vec<String>` — picks N random keys from shard, returns K with oldest `last_access`
- [x] 2.2 Implement `KVStore::evict_until_free(needed_bytes)` — iterates shards, calls `evict_candidates`, removes entries until `total_memory_bytes + needed_bytes <= max_memory_bytes`
- [x] 2.3 For `volatile-lru` and `volatile-ttl`: only sample keys from `StoredValue::Expiring` entries
- [x] 2.4 For `volatile-ttl`: sort candidates by nearest `expires_at` (evict soonest-to-expire first)
- [x] 2.5 For `allkeys-random` / `volatile-random`: select candidates at random without `last_access` comparison

## 3. Integrate Eviction into SET Path
- [x] 3.1 In `kv_store::set()`, before returning `MemoryLimitExceeded`, call `evict_until_free(entry_size)`
- [x] 3.2 If eviction freed enough space, proceed with insert; only return `MemoryLimitExceeded` when `noeviction` policy or eviction couldn't free enough
- [x] 3.3 Ensure eviction + insert happen within a single critical section to avoid over-insertion under concurrent load

## 4. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 4.1 Update `docs/analysis/synap-vs-redis/findings.md` — mark F-003 as resolved
- [x] 4.2 Write stress test: set `max_memory_mb=100`, write 200MB of distinct keys, assert `total_memory_bytes` stays near 100MB with no `MemoryLimitExceeded` errors
- [x] 4.3 Write test: `allkeys-lru` evicts least-recently-accessed keys first
- [x] 4.4 Write test: `volatile-lru` does not evict keys without TTL
- [x] 4.5 Write test: `noeviction` returns error when memory full (preserves current behavior)
- [x] 4.6 Run `cargo check` then `cargo test --workspace --all-features` and confirm all pass
