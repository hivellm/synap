# Memory Accounting & `maxmemory` (all datatypes)

This documents how Synap accounts memory across datatypes and enforces the
`maxmemory` limit (audit finding M-018).

## The problem it fixes

Before v1.0 the `maxmemory` budget tracked **only the KV store**. Hash, List,
Set, Sorted-Set, Stream and Queue memory was never counted, so a workload that
filled collections could blow far past the limit — eviction only shed KV strings
while everything else grew unbounded.

## Shared budget

All stores share one `GlobalMemory` budget (`synap_core::core::GlobalMemory`):

- Each store registers an `Arc<AtomicI64>` byte counter with the budget.
- `GlobalMemory::used()` is the **sum** of the registered counters — the true
  cross-datatype total.
- The KV eviction/refusal path consults this shared total, so evicting KV frees
  the budget for every datatype.

## How each datatype accounts

- **KV** updates its counter **live** on every mutation (it already tracked its
  own memory precisely).
- **Hash / List / Set / SortedSet / Stream / Queue** expose `memory_bytes()`
  (true payload size) and a `refresh_memory()` that recomputes their counter. A
  server background task calls `refresh_memory()` every 500 ms. This is
  **drift-free** (recomputed from actual contents) and avoids fragile
  per-mutation delta bookkeeping.

Accounted size is the payload (keys + values/members/elements/events); fixed
struct overhead is not included, so the accounted total is a lower bound on RSS.

## Enforcement

- **KV** writes (`SET`, `MSET`): when the shared total is over the cap, the
  configured eviction policy runs (evicting KV) or, under `noeviction`, the write
  is refused with `MemoryLimitExceeded`.
- **Hash / List / Set** grow writes (`HSET`/`HMSET`/`HSETNX`, `LPUSH`/`RPUSH`,
  `SADD`) call `would_exceed` and are **refused** with `MemoryLimitExceeded` when
  the shared total is over the cap.
- **SortedSet** `ZADD` returns `(usize, usize)` (not `Result`) and the
  count-capped **Stream/Queue** contribute to the accounted total (and are thus
  subject to eviction/refusal on the other write paths) but do not self-refuse.

Because collection counters are refreshed on an interval, the limit is a **soft**
limit with sub-second lag under a write burst — consistent with Redis's sampled
`maxmemory` behavior.

## Observability

`GET /metrics` exposes `synap_datatype_memory_bytes{datatype="kv|hash|list|set|
sorted_set|stream|queue"}`. Their sum is the total the eviction/refusal path uses,
so the accounted figure can be validated against process RSS.

## Not included here

Avoiding the full value **copy** on reads (`GET` currently does `to_vec()`) is the
separate, invasive read-path half of M-018 — tracked in
`phase9_kv-shared-buffer-reads` (store values behind `Arc<[u8]>`/`Bytes`).
