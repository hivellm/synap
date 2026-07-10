# Proposal: phase12_redis-parity-hotpath-perf

Source: docs/benchmarks/redis-vs-synap.md (live Redis 7 head-to-head)

## Why

The live head-to-head showed Synap at parity with Redis 7 at `-P 1` but trailing
under pipelining (`-P 16`) on several commands. Fresh sweep (Docker, same
network/server, `-n 200000 -c 50 -P 16`):

| op | Synap | Redis | ratio |
|---|--:|--:|--:|
| INCR | 480,769 | 943,396 | 0.51 |
| SADD | 630,915 | 873,362 | 0.72 |
| RPUSH | 722,022 | 925,926 | 0.78 |
| RPOP | 796,813 | 938,967 | 0.85 |
| SET | 775,194 | 900,901 | 0.86 |
| GET | 793,651 | 896,861 | 0.88 |
| LPOP | 793,651 | 900,901 | 0.88 |
| LPUSH | 722,022 | 500,000 | 1.44 (Synap wins) |

Redis is single-threaded and pays almost zero per-command overhead; Synap (tokio,
sharded stores) pays a synchronization + allocation tax per command. The goal is
to push every op as close to Redis as the multi-threaded architecture allows
(realistic ceiling ~0.9–1.0) and to document honestly where a gap is
architectural rather than fixable.

## Deep analysis — identified per-command costs

1. **Per-command String allocation in dispatch.** Both the RESP3 and SynapRPC
   dispatchers did `command.to_ascii_uppercase()` — one `String` alloc on EVERY
   command. Fixed by uppercasing into a stack buffer.
2. **INCR read-modify-write allocations.** `incr_unlocked` did
   `String::from_utf8(value.data().to_vec())` (a read-side `Vec` copy + UTF-8
   validation) and `key.to_string()` + a HashMap re-insert, all under the shard
   write lock (the hot-key serialization point). Fixed by parsing the `i64`
   straight from the byte slice and updating in place via `set_data`, which keeps
   the entry's TTL/variant.
3. **Collection-store stats lock.** `SetStore`/`ListStore`/`HashStore`/
   `SortedSetStore` keep `Arc<RwLock<Stats>>` and take `.write()` per mutating op
   just to bump a counter — a single global lock per datatype that every key's
   ops contend on (KVStore already uses lock-free `AtomicKVStats`). Candidate:
   convert the per-op counters to atomics.
4. **Arg-parsing allocations.** RESP3 `arg_str`/`arg_bytes` and the SynapRPC
   equivalents allocate a `String`/`Vec` per argument. `GET k` allocates the key
   `String`; `SET k v` allocates key + value. Redis parses in place.
5. **SADD/collection double-lookup.** `sadd` does `map.get(key)` (expiry check)
   then `map.entry(key.to_string())` — two lookups + a key alloc even when the
   key already exists.

## What Changes

Iterate: apply a batch of hot-path optimizations, rebuild the glibc bench image
(`scripts/Dockerfile.bench`), re-run the `redis-benchmark` + native SynapRPC
sweep, keep what moves the numbers, and update `docs/benchmarks/redis-vs-synap.md`
with before/after. Each change must keep all correctness tests green. Stop when
the remaining gaps are architectural (documented) rather than fixable.

## Impact

- Affected specs: none (internal hot-path optimization; wire formats unchanged)
- Affected code: `crates/synap-server/src/protocol/{resp3,synap_rpc}/**`,
  `crates/synap-core/src/core/{kv_store/store.rs, set.rs, list.rs, hash.rs,
  sorted_set.rs}`
- Breaking change: NO
- User benefit: higher pipelined throughput across GET/SET/INCR/list/set
  commands, closer to Redis 7
