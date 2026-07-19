# phase6g: shared cross-datatype memory budget via counter registry + periodic refresh
**Source**: manual
**Date**: 2026-07-09
**Related Task**: phase6g_v1-memory-accounting
**Tags**: analysis:synap-audit, phase6g, memory, eviction, maxmemory, architecture
phase6g (M-018) made maxmemory account for all datatypes, not just KV. Design that kept it tractable + low-risk:

- GlobalMemory (crates/synap-core/src/core/memory.rs): a registry of per-store Arc<AtomicI64> byte counters + a max_bytes cap. used() sums the registered counters; would_exceed(size) checks the cap. Clone-cheap (Arc<RwLock<Vec<...>>> inside).
- KEY trick to avoid editing KV's ~16 accounting sites: change AtomicKVStats.total_memory_bytes from AtomicI64 to Arc<AtomicI64>. Arc<AtomicI64> DEREFS to AtomicI64, so every existing .fetch_add/.load/.store call site compiles unchanged; only the field type + Default (Arc<T:Default>: Default) change. KVStore::with_global_memory registers this live counter. KV eviction/refusal checks now read the shared total via mem_used_and_max().
- Collections (hash/list/set/sorted_set/stream/queue): each gains mem: Option<GlobalMemory> + mem_bytes: Arc<AtomicI64> + with_global_memory + memory_bytes() (true payload sum) + refresh_memory() (store memory_bytes into counter). A 500ms server background task calls refresh_memory() on all of them. PERIODIC RECOMPUTE was chosen over per-mutation deltas: drift-free (recomputed from contents), no fragile delta-sign bugs. Downside: soft limit with sub-second lag (fine, like Redis sampling).
- Refusal: hash/list/set grow-writes (hset/hmset/hsetnx, lpush/rpush, sadd) call check_admit -> would_exceed -> Err(MemoryLimitExceeded). SortedSet zadd returns (usize,usize) not Result, so it can't self-refuse (counts toward total only); Stream/Queue are count-capped so also count-only.
- main.rs wiring gotcha: the AppState stores are NOT the tuple stores — set_store is ALWAYS rebuilt fresh at the authoritative binding (~line 402), ignoring the recovered one. Wire with_global_memory at the AUTHORITATIVE bindings (hash/list/set/sorted_set ~392-407) + stream_manager + the recovery tuple sites. Verify which instance actually reaches AppState.
- Metrics (1.5): synap_datatype_memory_bytes{datatype} set in update_broker_metrics from each store's memory_bytes() (kv via kv_store.stats().await.total_memory_bytes).
- Split out: the Arc<[u8]> read-copy-avoidance half of M-018 -> phase9_kv-shared-buffer-reads (invasive: StoredValue type + every read path + response boundary).
- Tests: budget_is_shared_across_datatypes (KV fills budget -> hash write refused) is the canonical M-018 validation. Note kv.set is async (#[tokio::test]).