# Findings — Synap vs Redis

Each finding has: title, evidence (file:line), impact, priority, estimated effort, and confidence.

---

## F-001: No binary protocol (RESP)

**Evidence**: [synap-server/src/server/handlers.rs](../../../synap-server/src/server/handlers.rs) (11,595 lines), [router.rs](../../../synap-server/src/server/router.rs). All endpoints are HTTP/JSON via Axum. No RESP parser exists in the codebase.

**Current state**: Every KV operation goes through:
1. HTTP parsing (headers, framing) — ~200-500 bytes overhead
2. JSON deserialization via `serde_json`
3. Business logic
4. JSON serialization of the response
5. HTTP framing on the way back

**Gap**: Redis RESP is binary, ~50-100 bytes overhead, native pipelining (client sends N requests without waiting, server reads in batch). Typical latency: RESP ~0.2ms, HTTP/JSON ~1-2ms.

**Impact**: **CRITICAL** — nullifies 80% of the sharding advantage. Maximum throughput is bound by parsing/serialization overhead, not lock contention.

**Priority**: P0 (blocks everything)
**Effort**: 3 weeks (RESP2 + RESP3 parser, separate TCP listener, mapping to internal handlers)
**Confidence**: High — verified by direct code reading

---

## F-002: No pipelining

**Evidence**: HTTP/1.1 synchronous request-response. No channel/buffer for batching multiple requests from the same client.

**Current state**: Each operation waits for the response before the next is sent. Even with keep-alive, there is no request multiplexing.

**Gap**: Redis pipelining lets a client queue 100+ commands before reading responses → 20-50x higher throughput for batch workloads.

**Impact**: **CRITICAL** — ETL workloads, bulk imports, cache warmup are orders of magnitude slower.

**Priority**: P0 (depends on F-001)
**Effort**: 2 weeks after RESP is in place
**Confidence**: High

---

## F-003: Eviction is not implemented — ✅ RESOLVED (phase1_implement-kv-eviction)

**Evidence**: [kv_store.rs:313-320](../../../synap-server/src/core/kv_store.rs#L313-L320) — checks `max_memory_mb` and returns `SynapError::MemoryLimitExceeded`. No eviction call. [types.rs:117-130](../../../synap-server/src/core/types.rs#L117-L130) declares `enum EvictionPolicy { None, LRU, LFU, TTL }` but only LRU has `last_access` tracking — and there is no loop that actually removes entries.

**Gap**: Redis has `maxmemory-policy` with 8 working strategies (allkeys-lru, allkeys-lfu, volatile-lru, volatile-ttl, allkeys-random, etc.). Synap has the enum but the logic is a stub.

**Resolution**: Implemented approximated LRU eviction with 6 Redis-compatible policies:
`noeviction` (default), `allkeys-lru`, `volatile-lru`, `allkeys-random`, `volatile-random`, `volatile-ttl`.
`evict_until_free(needed_bytes)` samples `eviction_sample_size` (default 5) keys per shard and evicts
the worst candidate per iteration until memory is freed. Integrated into both `set()` and `set_with_opts()`.
`EvictionPolicy` variants renamed to Redis kebab-case names (serialized: `"allkeys-lru"` etc.).

**Impact**: **RESOLVED** — Synap can now be used as a bounded cache. Under memory pressure, keys are evicted gracefully instead of returning errors (when policy ≠ noeviction).

**Priority**: P0
**Effort**: 2 weeks (implement approximated-LRU sampling like Redis, LFU with counter decay, integrate with existing TTL)
**Confidence**: High

---

## F-004: Write lock on GET to update LRU

**Evidence**: [kv_store.rs:371](../../../synap-server/src/core/kv_store.rs#L371) — `let mut data = shard.data.write()` inside the read path, just to update `last_access`.

**Current state**: Every GET acquires a write lock on the entire shard, even though it doesn't modify the data. Concurrent reads to the same shard are serialized.

**Gap**: 64-way sharding should allow parallel reads, but the write lock kills it. Redis doesn't have this problem because it's single-threaded.

**Solution**: Change `last_access: u32` to `AtomicU32` inside `StoredValue::Expiring`, update via `store(Relaxed)` while holding the read lock. Cost: ~4 extra bytes per entry (already aligned).

**Impact**: **HIGH** — in read-heavy workloads (typical cache), per-shard throughput drops to one reader at a time instead of N.

**Priority**: P0 (cheap fix, huge gain)
**Effort**: 3 days (localized change + stress tests)
**Confidence**: High

---

## F-005: `handlers.rs` is 11,595 lines

**Evidence**: `wc -l synap-server/src/server/handlers.rs` → 11595. Mixes endpoints for KV, Hash, List, Set, ZSet, Stream, Queue, PubSub, Auth, Transaction, Scripting, Monitoring.

**Impact**: **HIGH** for maintainability. Slow incremental compilation, frequent merge conflicts, code review impractical, hard to navigate.

**Priority**: P1 (doesn't block features but slows dev velocity)
**Effort**: 1 week (split per domain: `kv_handlers.rs`, `hash_handlers.rs`, etc., adjust `router.rs`)
**Confidence**: High

---

## F-006: Memory stats drift

**Evidence**: [kv_store.rs:328-334](../../../synap-server/src/core/kv_store.rs#L328-L334) — increments `memory_usage` on SET but there is no equivalent decrement on DELETE / expiration / eviction (when it eventually exists). Comment mentions "Estimated entry size" with no reconciliation. **The overwrite path doesn't update memory at all** — see [set-deep-dive.md S-05](set-deep-dive.md#s-05-memory-accounting-drifts-on-every-overwrite).

**Impact**: **HIGH** — reported stats (`INFO memory`) become progressively wrong. The longer the uptime, the worse it gets. Breaks autoscaling on memory metrics.

**Priority**: P1
**Effort**: 3 days (audit every path that removes entries, ensure decrement; ideally use `AtomicI64`)
**Confidence**: High

---

## F-007: MGET/MSET sequential across shards

**Evidence**: [kv_store.rs:512-521](../../../synap-server/src/core/kv_store.rs#L512-L521) — sequential loop over keys, each acquires its own shard separately.

**Gap**: With 64 shards, MGET on 64 distinct keys could parallelize 64 reads. Currently it's serial.

**Impact**: **MEDIUM** — batch operations waste the architectural parallelism.

**Priority**: P2
**Effort**: 1 week (group keys by shard, parallelize via `tokio::join!` or rayon)
**Confidence**: High

---

## F-008: No blocking operations (BLPOP/BRPOP/BZPOPMIN)

**Evidence**: [list.rs](../../../synap-server/src/core/list.rs), [sorted_set.rs](../../../synap-server/src/core/sorted_set.rs) — only non-blocking operations. Queue has nearby semantics but a different API.

**Gap**: Common Redis usage pattern (worker pulling jobs). Without it, clients have to actively poll.

**Impact**: **MEDIUM** — forces a client-side workaround and generates wasted traffic.

**Priority**: P2
**Effort**: 2 weeks (`tokio::sync::Notify` per list, timeout via `tokio::time::timeout`)
**Confidence**: High

---

## F-009: Pub/Sub without patterns (PSUBSCRIBE)

**Evidence**: [pubsub.rs](../../../synap-server/src/core/pubsub.rs) (649 lines) — exact topic matching only. No glob/regex matching. No keyspace notifications.

**Gap**: Redis supports `PSUBSCRIBE news.*` and `__keyspace@0__:user:*` for change notifications. Common cases: cache invalidation, event fan-out.

**Impact**: **MEDIUM** — incomplete feature coverage.

**Priority**: P2
**Effort**: 1 week (glob matching + keyspace notification hook in every mutating handler)
**Confidence**: High

---

## F-010: Cursor SCAN only on KV

**Evidence**: KV has SCAN; Hash/Set/ZSet don't have HSCAN/SSCAN/ZSCAN. Iteration only via SMEMBERS/HGETALL (load everything at once).

**Gap**: On large structures, "get all" operations hold the lock for tens of milliseconds. Cursor-based scan allows incremental iteration.

**Impact**: **MEDIUM** — limits the practical size of structures.

**Priority**: P2
**Effort**: 2 weeks (generic cursor design + per-type implementation)
**Confidence**: High

---

## F-011: No optimized allocator, no IO threads

**Evidence**: [Cargo.toml](../../../Cargo.toml) — no `mimalloc` or `jemallocator` dependency. Default Tokio runtime with no dedicated IO threads.

**Gap**: Redis 6.2+ has IO threads to offload parsing/serialization. mimalloc/jemalloc reduce fragmentation in cache workloads (small, varied allocations) by 15-30%.

**Impact**: **MEDIUM** — suboptimal performance under sustained load.

**Priority**: P3
**Effort**: mimalloc 1 day (one line of code + dep), IO threads 4 weeks (listener redesign)
**Confidence**: High for mimalloc, medium for IO threads (may require architectural change)

---

## F-012: KV `SET` hot path has ~20 distinct issues

**Evidence**: See [set-deep-dive.md](set-deep-dive.md) for the full breakdown.

**Summary**: The user's #1 reported pain point in real production deployments. Issues span correctness, performance, durability, and feature completeness:

- **No SET options** (NX/XX/GET/KEEPTTL/PX) — blocks distributed lock patterns (S-01)
- **Memory limit is racy and non-evicting** (S-03)
- **Global stats lock serializes every SET** (S-04)
- **Memory accounting drifts on overwrite** (S-05)
- **WAL is logged AFTER memory write** — durability lie (S-08)
- Plus 15 other issues: cloning, allocator pressure, JSON forcing, INCR destroying TTL, no max value size, etc.

**Impact**: **CRITICAL** — combined with F-003 (eviction), this is why Synap is currently not usable as a Redis replacement for cache workloads.

**Priority**: P0
**Effort**: 3-4 weeks for the 5 critical sub-issues, 6-7 weeks for all 20.
**Confidence**: Very high — every issue verified against the source.

---

## Summary by Priority

| Prio | Findings | Total effort |
|---|---|---|
| **P0** | F-001 (RESP), F-002 (pipelining), F-003 (eviction), F-004 (LRU lock), **F-012 (SET path)** | ~10-11 weeks |
| **P1** | F-005 (handlers.rs), F-006 (stats drift) | ~1.5 weeks |
| **P2** | F-007 (MGET parallel), F-008 (blocking), F-009 (PSUBSCRIBE), F-010 (SCAN) | ~6 weeks |
| **P3** | F-011 (allocator/IO threads) | ~4 weeks |

**Total estimated**: 21-22 weeks of focused work to close every identified gap. The first 4 weeks (F-012 + F-004) deliver disproportionate value: they fix the user's #1 pain point and unblock real cache use cases.
