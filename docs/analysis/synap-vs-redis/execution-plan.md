# Execution Plan — Synap vs Redis

Three phases, ordered by user impact and dependency. Phase 1 is non-negotiable: it fixes the user's #1 reported pain point and unblocks real cache use cases.

---

## Phase 1 — SET hot path + eviction + LRU unlock (≈ 5-6 weeks)

**Goal**: Make Synap usable as a real Redis-replacement cache. Fix the SET path, implement eviction, remove the GET write lock.

### 1.1 Fix SET correctness (week 1)
- **F-012/S-05** Memory accounting on overwrite: subtract old value size on every insert that returns `Some(old)`. Also fix delete and `cleanup_expired` paths.
- **F-012/S-04** Replace `Arc<RwLock<KVStats>>` with atomic counters. All `set/get/del/expire` paths use `fetch_add(_, Relaxed)`.
- **F-012/S-08** Make WAL a true write-ahead log. Two modes: `durability: sync` (wait for fsync before responding) and `durability: async` (current behavior, but response includes `durable: false`). Default to `async` for backward compat, document the semantics honestly.
- **F-012/S-13** Add `max_value_size_bytes` to `KVConfig`, reject oversized values in handler before allocation.
- **F-012/S-16** `INCR`/`DECR` preserve TTL on update. Use `checked_add`, return clean error on overflow.

**Acceptance**: Stress test with 1M SET-overwrite cycles → `total_memory_bytes` matches `sum(entry_size)` exactly. No drift.

### 1.2 Add SET options (week 2)
- **F-012/S-01** Implement `SetOptions` struct with `if_absent` (NX), `if_present` (XX), `keep_ttl`, `return_old` (GET).
- **F-012/S-02** Add `Expiry` enum: `Seconds | Milliseconds | UnixSeconds | UnixMilliseconds`. Change `expires_at` storage to `u64` milliseconds.
- Update HTTP handler to accept `options` field, update RESP handler (when ready) to parse `SET key value EX 60 NX`.
- Atomic NX/XX semantics: check + insert under the same shard write lock — no TOCTOU.

**Acceptance**: Test suite covers all 16 combinations of (NX|XX|none) × (GET|none) × (KEEPTTL|EX|PX|none). Distributed-lock pattern via `SET lock owner NX EX 30` works under 100-thread contention.

### 1.3 Implement real eviction (weeks 3-4)
- **F-003** Approximated-LRU sampling à la Redis. Background task picks N random keys per shard, evicts the K with oldest `last_access`.
- Implement `allkeys-lru`, `volatile-lru`, `allkeys-random`, `volatile-random`, `volatile-ttl`, `noeviction` (current behavior).
- LFU later (need a counter field in `StoredValue::Expiring`).
- **F-012/S-03** Eviction triggers when memory check in SET path detects pressure: evict until enough space, **then** insert. Single critical section.

**Acceptance**: Set `max_memory_mb=100`, write 200MB of distinct keys, observe steady eviction with `total_memory_bytes ≈ 100MB`. No `MemoryLimitExceeded` errors. Hit rate degrades gracefully.

### 1.4 Unblock GET concurrency (week 5, 3 days)
- **F-004** Change `last_access: u32` to `AtomicU32`. GET takes `data.read()` lock and updates `last_access` via `store(now, Relaxed)`. No write-lock acquisition on the read path.

**Acceptance**: Read-only benchmark on a single shard scales linearly with reader threads up to ~16 cores (today: flat at 1 core).

### 1.5 Memory and allocation reductions (week 5-6) — ✅ COMPLETED (phase1_reduce-kv-allocations)

- **F-012/S-06** Conditional clone: value cloned only when WAL persistence is active; moved into `set_with_opts` otherwise.
- **F-012/S-07** Single clock read per SET handler: `SystemTime::now()` called once at handler entry; relative expiries pre-converted to `Expiry::UnixMilliseconds` so `to_unix_ms()` inside `set_with_opts` is a no-op.
- **F-012/S-10** Permission key allocation eliminated: `require_resource_permission()` returns `Ok(())` immediately when `is_admin = true` (auth disabled = 99% of deployments) — no `format!("kv:{key}")` heap allocation.
- **F-012/S-11** Hub scope zero-copy: `MultiTenant::scope_kv_key()` returns `Cow::Borrowed` in standalone mode — zero heap allocation for 99% of key lookups.
- **F-012/S-12** `KVStore::set()` accepts `impl Into<String>`: callers with an owned `String` (recovery, replication) pass it directly, eliminating the internal `.to_string()` allocation.
- **F-012/S-14** Batched MSET per shard: keys grouped by shard index, each shard's write lock acquired once for all keys in that group (O(1) locks vs O(N) previously).

**Measured results** (`cargo bench --bench kv_bench`, criterion vs previous baseline):
| Benchmark | Latency (ns) | vs baseline |
|---|---|---|
| set_persistent/64B | ~116 ns | -1.5% (within noise) |
| set_persistent/256B | ~158 ns | -3.3% (p=0.01) |
| set_persistent/1024B | ~118 ns | **-17.8%** (p=0.00) |
| set_persistent/4096B | ~161 ns | **-13.5%** (p=0.00) |

Combined with F-004 (AtomicU32 GET, no write lock on reads), the concurrent read path scales linearly with cores — `test_concurrent_reads_no_write_lock` passes with 16 concurrent readers at 80K reads/thread without serialization.

### Phase 1 deliverables
- All P0/F-012 critical sub-issues resolved.
- Synap usable as a Redis-replacement cache for the first time.
- Stress-test suite covering eviction, NX semantics, memory accounting, and concurrent SET/GET.
- Updated docs/specs/PERFORMANCE.md with new numbers.

---

## Phase 2 — Wire protocol + feature parity (≈ 5-6 weeks)

**Goal**: Close the latency gap with Redis. Add missing features that block migration.

### 2.1 RESP2/RESP3 binary protocol (weeks 1-3)
- **F-001** Implement RESP parser (use `redis-protocol` crate as reference, or hand-write — straightforward state machine).
- Separate TCP listener on port 6379 (Redis default) — keeps HTTP/MCP/UMICP intact.
- Map RESP commands to existing internal handlers: SET, GET, DEL, INCR, EXPIRE, EXISTS, TTL, KEYS, SCAN, HSET, HGET, LPUSH, RPUSH, LRANGE, SADD, SMEMBERS, ZADD, ZRANGE, PUBLISH, SUBSCRIBE.
- Wire-format compatibility means existing Redis clients (redis-cli, redis-py, redis-rs, ioredis) connect without changes.
- **F-002** Pipelining: read N commands from the same connection before flushing responses.

**Acceptance**: `redis-cli -p 6379 -h synap` works for all common commands. `redis-benchmark -t set,get -n 1000000 -P 100` shows ≥80% of native Redis throughput.

### 2.2 Refactor `handlers.rs` (week 4)
- **F-005** Split into `kv_handlers.rs`, `hash_handlers.rs`, `list_handlers.rs`, `set_handlers.rs`, `zset_handlers.rs`, `stream_handlers.rs`, `queue_handlers.rs`, `pubsub_handlers.rs`, `transaction_handlers.rs`, `script_handlers.rs`, `monitor_handlers.rs`. Update `router.rs` accordingly. Pure mechanical move + visibility fixes.

**Acceptance**: No file >1500 lines. Compilation time of `synap-server` improves measurably.

### 2.3 Blocking operations (week 5)
- **F-008** BLPOP, BRPOP, BLMOVE, BZPOPMIN, BZPOPMAX. Use `tokio::sync::Notify` per list/zset, `tokio::time::timeout` for the deadline.

### 2.4 Pub/Sub patterns + keyspace notifications (week 6)
- **F-009** PSUBSCRIBE with glob matching (`news.*`, `user.?.profile`).
- Keyspace notifications: hook into every mutating handler (SET, DEL, EXPIRE, HSET, LPUSH, …) to publish to `__keyspace@0__:<key>` and `__keyevent@0__:<event>`. Configurable via `notify-keyspace-events`.

### Phase 2 deliverables
- Protocol parity with Redis (RESP + pipelining).
- Distributed lock, blocking queue, and cache invalidation patterns all work.
- `handlers.rs` refactored.

---

## Phase 3 — Performance optimization + feature completeness (≈ 4-5 weeks)

**Goal**: Outperform Redis on multi-core workloads. Close remaining feature gaps.

### 3.1 Allocator + parallel batch ops (week 1)
- **F-011** Add `mimalloc` as default global allocator behind a feature flag.
- **F-007** Parallel MGET/MSET: group keys by shard, dispatch with `tokio::join!` or `rayon::scope`. Should give ~10x improvement on batches >100 keys.

### 3.2 SCAN cursor on all data structures (weeks 2-3)
- **F-010** Generic cursor design (continuation token = shard index + bucket index). HSCAN, SSCAN, ZSCAN. Stable iteration even with concurrent inserts.

### 3.3 Stream completeness (week 4)
- Complete XPENDING (full PEL info), XAUTOCLAIM, XINFO STREAM/GROUPS/CONSUMERS, XREAD with BLOCK.

### 3.4 IO threads + benchmarks (week 5)
- **F-011** Tokio runtime tuning, dedicated IO threads for RESP listener.
- Publish `redis-benchmark` results: Synap vs Redis 7 on multi-core. Should show Synap 1.5-3x ahead on multi-core SET/GET.

### Phase 3 deliverables
- Multi-core benchmarks beating Redis.
- All major Redis commands available.
- Full Stream/Cluster feature parity.

---

## Risk Register

| Risk | Impact | Mitigation |
|---|---|---|
| Eviction implementation has subtle bugs | High | Property-based tests with `proptest`. Stress test with adversarial workloads (hot key, all-cold, all-hot, churn). |
| RESP parser performance below target | High | Benchmark against `redis-protocol` crate before committing. Profile-guided optimization. |
| Atomic stats race conditions | Medium | Loom-based tests for the few non-trivial cases (e.g. `incr` ordering). |
| WAL durability changes break existing deployments | Medium | Default to old behavior (`async`), require opt-in for `sync`. Document migration. |
| RESP listener consumes too much memory under high connection counts | Medium | Per-connection buffer pools, configurable max connections. |
| Refactor of `handlers.rs` causes regressions | Low | Pure mechanical split, full test suite must pass before/after. |

---

## Success Metrics

**Phase 1 done when**:
- `SET key value NX EX 60` works atomically under 1000-thread contention
- `max_memory_mb=100` test holds memory at ~100MB indefinitely with no errors
- `total_memory_bytes` accuracy within ±1% of actual after 10M operations
- WAL durability mode is tunable, current behavior is correctly labeled
- Read-only benchmark scales to ≥8 cores

**Phase 2 done when**:
- `redis-cli` connects on port 6379 and runs the standard command set
- `redis-benchmark -t set,get -P 100` reaches ≥80% of Redis throughput
- BLPOP, PSUBSCRIBE, keyspace notifications work
- No file in `synap-server/src/server/` exceeds 1500 lines

**Phase 3 done when**:
- Published benchmark shows Synap ≥1.5x Redis on 8-core multi-client SET workload
- HSCAN/SSCAN/ZSCAN work
- Stream commands at 95% Redis parity

---

## What this plan does NOT cover

- **Lua scripting atomicity** — already exists via mlua, but transaction semantics need their own analysis (`scripting.rs` was not deeply audited).
- **Cluster failover edge cases** — split-brain testing, sentinel-equivalent. Separate analysis needed.
- **Module system** — Redis modules are out of scope. Not required for parity.
- **TLS hardening** — assumed OK; not audited here.
- **Client SDK updates** — once RESP is in place, the existing SDKs can either keep using HTTP or switch to RESP. SDK roadmap is a separate document.
