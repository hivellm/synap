# KV `SET` Deep Dive — Every Issue in the Hot Path

This document catalogs **every problem** found in the KV `SET` code path, from the HTTP handler down to the shard insert. The user reports this as the #1 pain point in real Synap deployments. Each issue has: location (file:line), description, impact, and proposed fix.

The full SET path under analysis:

- HTTP handler: [synap-server/src/server/handlers.rs:228-270](../../../synap-server/src/server/handlers.rs#L228-L270)
- KV core: [synap-server/src/core/kv_store.rs:301-349](../../../synap-server/src/core/kv_store.rs#L301-L349)
- Stored value: [synap-server/src/core/types.rs:7-58](../../../synap-server/src/core/types.rs#L7-L58)

---

## S-01: SET request struct has zero options (no NX/XX/GET/KEEPTTL/PX)

**Location**: [handlers.rs:58-63](../../../synap-server/src/server/handlers.rs#L58-L63), [kv_store.rs:301](../../../synap-server/src/core/kv_store.rs#L301)

```rust
pub struct SetRequest {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl: Option<u64>,
}

pub async fn set(&self, key: &str, value: Vec<u8>, ttl_secs: Option<u64>) -> Result<()>
```

**Problem**: Synap's SET only accepts `(key, value, ttl_seconds)`. Redis SET supports:

| Option | Meaning | Why it matters |
|---|---|---|
| `NX` | Only set if key does not exist | **Foundational primitive for distributed locks**. Without this, distributed locks via Synap are racy. |
| `XX` | Only set if key already exists | Refresh-only patterns, optimistic updates |
| `GET` | Return old value (atomic GETSET) | Atomic swap, queue handoff, leader election |
| `KEEPTTL` | Preserve existing TTL on overwrite | Cache refresh without losing expiry |
| `EX <s>` / `PX <ms>` | Seconds / milliseconds TTL | **Sub-second TTL** for rate limiters, idempotency keys |
| `EXAT <ts>` / `PXAT <ts>` | Absolute Unix expiry time | Precise scheduled expiry |
| `IFEQ <value>` (Redis 8) | Set only if current value equals X | Optimistic CAS without scripting |

**Impact**: **CRITICAL — this is the user-visible #1 issue.** A real production cache without `SET NX` cannot be used for distributed locking, idempotency tokens, leader election, or any pattern that requires atomic create-if-absent. Today, Synap clients have to implement these via `GET → check → SET`, which has a TOCTOU race window.

**Fix**: Extend `SetRequest` and `KVStore::set` with a `SetOptions` struct:

```rust
pub struct SetOptions {
    pub if_absent: bool,    // NX
    pub if_present: bool,   // XX
    pub keep_ttl: bool,
    pub return_old: bool,   // GET
    pub expiry: Option<Expiry>,
}
pub enum Expiry {
    Seconds(u64),
    Milliseconds(u64),
    UnixSeconds(u64),
    UnixMilliseconds(u64),
}
pub async fn set(&self, key: &str, value: Vec<u8>, opts: SetOptions)
    -> Result<SetOutcome>;
pub enum SetOutcome { Set, NotSet, ReturnedOld(Vec<u8>) }
```

The implementation must do the NX/XX check **inside the same write-lock acquisition** as the insert — no GET-then-SET split, no TOCTOU.

---

## S-02: TTL granularity is seconds-only

**Location**: [kv_store.rs:301](../../../synap-server/src/core/kv_store.rs#L301), [types.rs:14](../../../synap-server/src/core/types.rs#L14)

```rust
ttl_secs: Option<u64>
expires_at: u32,  // Unix timestamp (valid until year 2106)
```

**Problem**: Synap stores expiry as `u32` Unix seconds. There is no way to express "expire in 250ms" — common for rate limiters (sliding window per HTTP request), idempotency tokens (5-minute TTL is fine, 5-second is also fine, but 500ms isn't expressible), and circuit breakers.

**Impact**: HIGH. Forces clients to round up to 1 second, which doesn't work for sub-second use cases at all.

**Fix**: Change `expires_at` to `u64` milliseconds (or add a `Millis` variant). 8 bytes per Expiring entry (vs 4) — but that's the same overhead Redis pays. The 2106 limit goes away as a bonus.

---

## S-03: Memory limit check is racy AND non-evicting

**Location**: [kv_store.rs:310-321](../../../synap-server/src/core/kv_store.rs#L310-L321)

```rust
{
    let stats = self.stats.read();
    let max_bytes = self.config.max_memory_mb * 1024 * 1024;
    if stats.total_memory_bytes + entry_size > max_bytes {
        warn!("Memory limit exceeded: {}/{}", ...);
        return Err(SynapError::MemoryLimitExceeded);
    }
}

let shard = self.get_shard(key);
let mut data = shard.data.write();
let is_new = data.insert(key.to_string(), stored).is_none();
```

**Problem**: Two compounding bugs.

**Bug A — race**: The memory check holds a `read()` lock on stats, releases it, then acquires the shard `write()` lock and inserts. Between these two steps, N other threads can pass the check and insert, blowing through the limit. Under concurrent SET load, the limit is a soft suggestion, not a guarantee.

**Bug B — non-evicting**: When the limit is hit, Synap **returns an error to the client** instead of evicting cold keys. A cache that returns errors when full is not a cache — it's a key-value store with a hard ceiling. This is the **single biggest reason Synap can't be used as a Redis replacement** for cache workloads. Every Redis user expects `maxmemory-policy allkeys-lru` to silently evict; Synap gives them `MemoryLimitExceeded`.

**Impact**: **CRITICAL.** Combined with S-01 (no NX), this is why real-world Synap deployments hit a wall.

**Fix**: 
1. Implement actual eviction. When `total_memory_bytes + entry_size > max_bytes`, evict using the configured policy (LRU/LFU/random/ttl) until enough space is freed, **then** insert.
2. The check + eviction + insert must happen atomically or be protected by a single mutex (a per-shard memory accounting + a global "evictor" task is the Redis approach — approximated LRU sampling).
3. Returning an error should only happen with `EvictionPolicy::None` (explicit opt-in to fail-on-full).

---

## S-04: Stats lock is a global serialization point on every SET

**Location**: [kv_store.rs:329-334](../../../synap-server/src/core/kv_store.rs#L329-L334)

```rust
let mut stats = self.stats.write();
stats.sets += 1;
if is_new {
    stats.total_keys += 1;
    stats.total_memory_bytes += entry_size;
}
```

**Problem**: `self.stats` is a single `Arc<RwLock<KVStats>>` shared across all 64 shards. **Every SET acquires a global write lock on it.** This completely defeats the purpose of 64-way sharding for write workloads — even though shards 0..63 don't contend for data, they all contend for stats.

**Impact**: **CRITICAL** for write throughput. Profile any SET-heavy workload and `stats.write()` will be the top bottleneck. Order of magnitude perf loss vs what the architecture promises.

**Fix**: Replace `KVStats` with atomic counters.

```rust
#[derive(Default)]
pub struct KVStats {
    pub total_keys: AtomicUsize,
    pub total_memory_bytes: AtomicUsize,
    pub gets: AtomicU64,
    pub sets: AtomicU64,
    pub dels: AtomicU64,
    pub hits: AtomicU64,
    pub misses: AtomicU64,
}
```

All updates become `stats.sets.fetch_add(1, Relaxed)` — lock-free, no contention. The `Clone` impl for snapshotting needs to be changed to a `snapshot()` method that loads each atomic.

---

## S-05: Memory accounting drifts on every overwrite

**Location**: [kv_store.rs:326-334](../../../synap-server/src/core/kv_store.rs#L326-L334)

```rust
let is_new = data.insert(key.to_string(), stored).is_none();
// ...
if is_new {
    stats.total_keys += 1;
    stats.total_memory_bytes += entry_size;
}
```

**Problem**: When `is_new == false` (overwrite), `total_memory_bytes` is not updated **at all**. The old value's size is never subtracted, the new value's size is never added. Concrete sequence:

1. `SET k "small"` → `total_memory_bytes += 5`
2. `SET k <1MB blob>` → `total_memory_bytes += 0` (BUG: should be +1MB - 5)
3. `SET k "tiny"` → `total_memory_bytes += 0` (BUG: should be -almost 1MB)

After a few overwrites, `total_memory_bytes` is meaningless. This breaks:
- The memory limit check in S-03 (uses bogus value)
- Monitoring / autoscaling based on `INFO memory`
- Any eviction policy that uses memory pressure as a trigger

**Impact**: **CRITICAL.** This bug actively poisons every other memory-related feature.

**Fix**: `data.insert` returns `Option<StoredValue>`. Use it:

```rust
let old = data.insert(key.to_string(), stored);
let old_size = old.as_ref().map(|v| key.len() + v.data().len() + size_of::<StoredValue>()).unwrap_or(0);
stats.total_memory_bytes
    .fetch_add(entry_size, Relaxed)
    .fetch_sub(old_size, Relaxed);
if old.is_none() {
    stats.total_keys.fetch_add(1, Relaxed);
}
```

The `delete` and `cleanup_expired` paths have the same bug — they decrement `total_keys` ([kv_store.rs:382, 424, 618](../../../synap-server/src/core/kv_store.rs#L382)) but never decrement `total_memory_bytes`. Same fix needed there.

---

## S-06: Value is cloned unconditionally even when cache is disabled

**Location**: [kv_store.rs:307, 345](../../../synap-server/src/core/kv_store.rs#L307)

```rust
let stored = StoredValue::new(value.clone(), ttl_secs);  // line 307
// ...
if let Some(ref cache) = self.cache {
    cache.put(key.to_string(), value, cache_ttl);  // line 345 — uses original
}
```

**Problem**: `value.clone()` on line 307 is needed only because the original `value` is consumed by `cache.put` later. **If the cache is disabled** (the common case for users who only want the main store), the clone is pure waste — a full copy of the value buffer for nothing.

For a 1MB value with cache disabled: 1MB of allocator pressure per SET, plus a memcpy.

**Impact**: HIGH for large-value workloads. Doubles peak memory and allocation rate during SET.

**Fix**: Only clone when cache is enabled:

```rust
let stored = if self.cache.is_some() {
    StoredValue::new(value.clone(), ttl_secs)
} else {
    StoredValue::new(value, ttl_secs)
};
```

Or refactor to pass `Arc<[u8]>` so both the cache and the store share the buffer without copying. Even better: make `StoredValue::data` an `Arc<[u8]>` end-to-end and amortize across overwrites.

---

## S-07: Three clock reads per SET

**Location**: [types.rs:25, 38](../../../synap-server/src/core/types.rs#L25), [kv_store.rs:339](../../../synap-server/src/core/kv_store.rs#L339)

`StoredValue::new` calls `current_timestamp()` (1 syscall via vDSO). Then `kv_store::set` calls `SystemTime::now()` again to compute `cache_ttl` (line 339). Internally that's a third call when the cache layer also stamps its own TTL.

**Problem**: vDSO clock reads are cheap (~20-50ns) but still measurable at high SET rates. More importantly, the **three reads can return different timestamps**, so the value's `expires_at` and the cache's TTL can be off by a millisecond or more.

**Impact**: MEDIUM. Performance loss is small (~100ns/SET); correctness drift is small but real.

**Fix**: Read the clock once at the top of `set()`, pass the timestamp down to `StoredValue::new` and to `cache.put`. While at it, consider a coarse monotonic clock (`std::time::Instant`) for LRU since absolute Unix time isn't needed for ordering — and `Instant::now()` is faster than `SystemTime::now()` on Linux.

---

## S-08: WAL is logged AFTER the in-memory write — durability lie

**Location**: [handlers.rs:250-264](../../../synap-server/src/server/handlers.rs#L250-L264)

```rust
state.kv_store.set(&scoped_key, value_bytes.clone(), req.ttl).await?;

if let Some(ref persistence) = state.persistence {
    if let Err(e) = persistence.log_kv_set(scoped_key, value_bytes, req.ttl).await {
        error!("Failed to log KV SET to WAL: {}", e);
        // Don't fail the request, data is already in memory
    }
}

Ok(Json(SetResponse { success: true, key: req.key }))
```

**Problem**: This is **not** a write-ahead log. The data is written to memory first, then logged. If the process crashes between the in-memory write and the WAL append, the SET is silently lost despite the client receiving HTTP 200 + `{"success": true}`. The comment "Don't fail the request, data is already in memory" makes the bug explicit.

This is a **durability correctness bug**, not a perf issue. Clients have no way to know whether their write was actually durable.

**Impact**: **CRITICAL** for any user who treats Synap as durable. The whole point of a WAL is "log first, apply second."

**Fix**: Two options.
1. **True write-ahead**: Append to WAL → fsync (or wait for group commit ACK) → apply to memory → return success. Adds latency but is honest.
2. **Tunable durability** (Redis approach): Per-request flag `durability: { sync, async, none }`. `async` is the current behavior but the response should include `durable: false`. `sync` waits for fsync. `none` skips WAL entirely.

Either way, the current behavior (claim success before WAL append) needs to stop or be loudly documented as "best-effort durability."

---

## S-09: SET takes a `serde_json::Value` and forces JSON encoding

**Location**: [handlers.rs:60-61, 246-247](../../../synap-server/src/server/handlers.rs#L60-L61)

```rust
pub struct SetRequest {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl: Option<u64>,
}
// ...
let value_bytes = serde_json::to_vec(&req.value)
    .map_err(|e| SynapError::SerializationError(e.to_string()))?;
```

**Problem**: The HTTP API forces values through `serde_json::Value`. Consequences:

1. **Strings get quoted**: `SET k "hello"` actually stores the bytes `"hello"` (5 chars + 2 quotes = 7 bytes). Reading back via a non-Synap-aware tool gets `"hello"` literally.
2. **Binary data is hostile**: Want to store a JPEG? You must base64-encode it client-side, which inflates by 33% AND forces the client into a specific encoding scheme. Then `to_vec` re-quotes the base64 string. Total overhead: ~1.6x.
3. **Nested objects get re-encoded**: Client sends `{"value": {"a": 1}}`, server stores `{"a":1}` as bytes. The serde_json parse + reserialize is pure waste — the bytes existed in the request body and could have been used directly.
4. **Type info is lost**: Storing `42` as JSON gives bytes `42`, but reading it back via [the GET handler](../../../synap-server/src/server/handlers.rs#L297-L304) defaults to UTF-8 string decode, so the client gets `"42"` not `42`.
5. **Throughput**: Every SET incurs `serde_json::to_vec` allocation + parsing.

**Impact**: HIGH. Footgun for binary data, poor performance, and the round-trip semantics are surprising.

**Fix**:
1. Add a separate raw-bytes endpoint: `POST /kv/raw/:key` with `Content-Type: application/octet-stream`, body = raw bytes. No JSON involved.
2. For JSON convenience endpoint, store the raw request body bytes directly (Axum gives access via `Bytes` extractor), no `serde_json::Value` round-trip.
3. Track value type (`raw` | `json` | `string` | `int`) as a single byte alongside the value, so GET can return the right type.

The RESP protocol implementation (F-001) will sidestep this entirely, but the HTTP API needs to be fixed in the meantime.

---

## S-10: Permission check allocates a String per SET

**Location**: [handlers.rs:239](../../../synap-server/src/server/handlers.rs#L239)

```rust
require_permission(&ctx, &format!("kv:{}", req.key), Action::Write)?;
```

**Problem**: `format!("kv:{}", req.key)` allocates a fresh `String` for every SET, just to feed the permission check. For a 32-byte key, that's a heap allocation per request.

**Impact**: MEDIUM. Allocator pressure under high load.

**Fix**: Either pass the key + namespace separately to `require_permission` (no concat), or use a small `SmallVec`/stack buffer for keys < 256 bytes. Or define ACL resources as `(namespace, key)` tuples.

---

## S-11: Hub multi-tenant scoping allocates even when Hub is off

**Location**: [handlers.rs:243-244](../../../synap-server/src/server/handlers.rs#L243-L244)

```rust
let scoped_key =
    crate::hub::MultiTenant::scope_kv_key(hub_ctx.as_ref().map(|c| c.user_id()), &req.key);
```

**Problem**: Even when Hub mode is disabled (`hub_ctx is None`), `scope_kv_key` returns an owned `String` — likely just `req.key.to_string()`, which is still a heap alloc. On the disabled-Hub fast path, the function should return `&str` borrowed from the original.

**Impact**: MEDIUM. One allocation per SET that should be zero in the common case.

**Fix**: Change `scope_kv_key` to return `Cow<'_, str>`. Borrowed when Hub is off, owned when Hub is on.

---

## S-12: `key.to_string()` again inside the shard insert

**Location**: [kv_store.rs:326](../../../synap-server/src/core/kv_store.rs#L326)

```rust
let is_new = data.insert(key.to_string(), stored).is_none();
```

**Problem**: The handler already passed an owned `scoped_key: String`, then re-borrowed as `&str` to call `set(&scoped_key, ...)`. Now `set` clones it back to `String` for the insert. Path: `String → &str → String`. That's a redundant allocation.

**Impact**: LOW per call but multiplied by SET rate.

**Fix**: Change `set` to take `key: impl Into<String>` or `key: String`. Same applies to `delete`, `expire`, etc.

---

## S-13: No size limit per value

**Location**: [kv_store.rs:301-349](../../../synap-server/src/core/kv_store.rs#L301-L349) — no validation

**Problem**: A client can SET a 10 GB value and the server will attempt to store it (until OOM). There is no `max_value_size` config equivalent to Redis `proto-max-bulk-len` (default 512 MB).

**Impact**: HIGH. Denial-of-service vector. A single misbehaving client can OOM the server.

**Fix**: Add `max_value_size_bytes` to `KVConfig` (default 64 MB or so), reject in handler before parsing the body, and reject in `set()` before allocating.

---

## S-14: No batched SET that groups by shard

**Location**: [kv_store.rs:501-509](../../../synap-server/src/core/kv_store.rs#L501-L509)

```rust
pub async fn mset(&self, pairs: Vec<(String, Vec<u8>)>) -> Result<()> {
    debug!("MSET count={}", pairs.len());
    for (key, value) in pairs {
        self.set(&key, value, None).await?;
    }
    Ok(())
}
```

**Problem**: `mset` is a literal loop calling `set` for each pair. For 1000 keys this acquires 1000 shard locks + 1000 stats locks (= 2000+ lock ops). With proper sharded batching, you could acquire each shard's lock at most once and insert all keys destined for that shard in a single locked region.

**Impact**: HIGH for batch workloads. With 64 shards and 1000 keys, ideal is ~64 lock acquisitions, current is ~2000.

**Fix**:

```rust
pub async fn mset(&self, pairs: Vec<(String, Vec<u8>)>) -> Result<()> {
    let mut by_shard: Vec<Vec<(String, Vec<u8>)>> = vec![Vec::new(); SHARD_COUNT];
    for (k, v) in pairs {
        by_shard[self.shard_for_key(&k)].push((k, v));
    }
    for (i, batch) in by_shard.into_iter().enumerate() {
        if batch.is_empty() { continue; }
        let mut data = self.shards[i].data.write();
        for (k, v) in batch {
            data.insert(k, StoredValue::new(v, None));
        }
    }
    Ok(())
}
```

This becomes one lock per shard instead of one lock per key. Combined with S-04 (atomic stats), MSET throughput should improve by ~10-50x for large batches.

---

## S-15: No write coalescing for hot keys

**Location**: SET path generally

**Problem**: If 10 threads SET the same key in 1ms, all 10 acquire the same shard's write lock sequentially. The first 9 writes are immediately overwritten by the 10th — pure waste of lock time.

**Impact**: MEDIUM for hot-key workloads (counters, top-of-feed, leaderboard tips).

**Fix** (advanced): A small per-shard coalescing buffer. Writes to the same key within a 100µs window collapse to the latest value before the actual insert. Adds latency for the first writer but improves throughput dramatically. Trade-off; ship without it first, add if benchmarks justify.

---

## S-16: `incr`/`decr` re-implements the SET race instead of using SetOptions

**Location**: [kv_store.rs:463-498](../../../synap-server/src/core/kv_store.rs#L463-L498)

```rust
pub async fn incr(&self, key: &str, amount: i64) -> Result<i64> {
    let shard = self.get_shard(key);
    let mut data = shard.data.write();

    let current_value = if let Some(value) = data.get(key) {
        // ... parse as i64 ...
    } else { 0 };

    let new_value = current_value + amount;
    data.insert(key.to_string(), StoredValue::new(new_value.to_string().into_bytes(), None));

    let mut stats = self.stats.write();
    stats.sets += 1;
    Ok(new_value)
}
```

**Problem**: `incr` correctly holds the shard write lock for the read+modify+write (good — atomic within the shard). But:

1. **Wraparound on overflow**: `current_value + amount` will panic in debug, wrap in release. Should use `checked_add` and return a clean error.
2. **TTL is destroyed on every INCR**: `StoredValue::new(..., None)` resets TTL to nothing. If the user did `SET counter 1 EX 60` and then `INCR counter`, the TTL is gone. Redis preserves TTL on INCR.
3. **Same stats lock contention as S-04**.

**Impact**: HIGH (TTL destruction) + MEDIUM (overflow).

**Fix**: Read the old `StoredValue`, compute the new int with `checked_add`, build a new `StoredValue` that preserves the old `expires_at` if it was Expiring. Use atomic stats.

---

## S-17: SET cluster routing check fires even with cluster disabled

**Location**: [kv_store.rs:221-267, 305](../../../synap-server/src/core/kv_store.rs#L221-L267)

```rust
fn check_cluster_routing(&self, key: &str) -> Result<()> {
    if let Some(ref topology) = self.cluster_topology {
        // ... full check ...
    } else {
        Ok(())
    }
}
// ...
self.check_cluster_routing(key)?;  // every SET, GET, DEL
```

**Problem**: With cluster mode disabled, this is a single Option check + branch — cheap. But it's still a function call (likely inlined, but not guaranteed) and a `?` propagation point on every SET. More importantly, when cluster IS enabled, the routing computation runs **on every operation**, including reading the topology Arc.

**Impact**: LOW with cluster off, MEDIUM with cluster on.

**Fix**: When cluster is disabled, the field could be a `()` via a generic parameter, eliminating the runtime check entirely. Or use a const generic. Or accept the small cost.

For cluster-on path: cache the slot owner per shard for ~100ms to avoid recomputing on every op.

---

## S-18: SET on overwrite implicitly resets LRU position

**Location**: [kv_store.rs:307-326](../../../synap-server/src/core/kv_store.rs#L307-L326)

**Problem**: When SET overwrites an existing key, a fresh `StoredValue` is built, with `last_access = now()`. So overwriting a key counts as "recently accessed" for LRU. This is **probably** the right semantic but isn't documented and Redis differs subtly: in Redis, overwriting a key updates LRU but the LFU counter inherits from the old entry. Synap's behavior loses LFU context (when LFU is eventually implemented).

**Impact**: LOW. Document or align with Redis.

**Fix**: When LFU is implemented, preserve the counter on overwrite.

---

## S-19: SET response is needlessly large JSON

**Location**: [handlers.rs:65-69, 266-269](../../../synap-server/src/server/handlers.rs#L65-L69)

```rust
pub struct SetResponse {
    pub success: bool,
    pub key: String,
}
// ...
Ok(Json(SetResponse { success: true, key: req.key }))
```

**Problem**: Echoing the key back in every SET response wastes bandwidth (the client already knows the key it just sent). For SET-heavy workloads with long keys (e.g., 200-byte UUIDs), this doubles the response size for no value.

**Impact**: LOW-MEDIUM. Wasted bandwidth and serialization time.

**Fix**: Drop `key` from the response. The 204 No Content status would be even better when there's nothing to return — Redis returns `OK` (3 bytes) for SET.

---

## S-20: No metric for SET latency / failure modes

**Location**: SET path generally

**Problem**: There's a `stats.sets` counter (SET count) but no histogram of SET latency, no breakdown by failure cause (memory limit vs cluster redirect vs WAL fail). Operators can't tell why SET is slow or failing.

**Impact**: MEDIUM for production observability.

**Fix**: Add Prometheus metrics: `synap_kv_set_duration_seconds` (histogram), `synap_kv_set_total{status="ok|memory_full|moved|ask|wal_fail"}` (counter).

---

## Summary Table

| ID | Issue | Severity | Effort |
|---|---|---|---|
| S-01 | No SET options (NX/XX/GET/KEEPTTL/PX) | **CRITICAL** | 1 week |
| S-02 | TTL granularity is seconds-only | HIGH | 2 days |
| S-03 | Memory limit racy + non-evicting | **CRITICAL** | 2 weeks (depends on eviction) |
| S-04 | Stats lock global serialization | **CRITICAL** | 2 days |
| S-05 | Memory accounting drifts on overwrite | **CRITICAL** | 1 day |
| S-06 | Value cloned even when cache off | HIGH | 1 day |
| S-07 | Three clock reads per SET | MEDIUM | 1 day |
| S-08 | WAL after memory write — durability lie | **CRITICAL** | 1 week |
| S-09 | `serde_json::Value` API forces JSON | HIGH | 3 days |
| S-10 | Permission check allocates String | MEDIUM | 1 day |
| S-11 | Hub scoping allocates when Hub off | MEDIUM | 1 day |
| S-12 | Redundant `key.to_string()` | LOW | 1 day |
| S-13 | No max value size limit | HIGH (DoS) | 1 day |
| S-14 | MSET not batched per shard | HIGH | 2 days |
| S-15 | No hot-key write coalescing | MEDIUM | 1 week (optional) |
| S-16 | INCR destroys TTL + can overflow | HIGH | 1 day |
| S-17 | Cluster routing on every op | LOW | 2 days |
| S-18 | LRU semantics on overwrite undocumented | LOW | doc |
| S-19 | SET response echoes key | LOW | 1 hour |
| S-20 | No SET latency metrics | MEDIUM | 1 day |

**Critical items (S-01, S-03, S-04, S-05, S-08)**: ~3-4 weeks of focused work to fix the SET hot path.
**All items combined**: ~6-7 weeks.

These five critical items are the **single highest-leverage work in the entire project**. They fix the user's #1 reported pain point, eliminate a durability lie, restore correctness of memory accounting, and unlock the throughput the sharding architecture promises.
