# Proposal: phase7_v1-redis-benchmark-and-perf

Source: docs/analysis/synap-v1-release/ (F-016, F-017, F-018, F-019); reconciles docs/analysis/synap-vs-redis/

## Why
The prior `synap-vs-redis` analysis is partly outdated: its CRITICAL blockers (no RESP
protocol, no pipelining, write-lock on GET, no eviction) are all shipped — RESP3 with a
94-command dispatch listens on 6379, SynapRPC (MessagePack) on 15501, GET uses an AtomicU32
fast path, and 6 Redis eviction policies exist. But no published benchmark proves the result,
and a handful of parity/perf gaps remain undecided (blocking ops, PSUBSCRIBE/keyspace
notifications, SCAN cursors, allocator, IO threads, LFU, parallel MGET/MSET). v1.0 needs
hard numbers against Redis 7 and an explicit ship/defer decision per remaining gap — and the
old analysis docs must be annotated so future implementers stop trusting stale findings.

## What Changes
1. Annotate `docs/analysis/synap-vs-redis/` findings with current status (RESOLVED/OPEN +
   evidence) so the document stops misleading implementers.
2. Benchmark suite: `redis-benchmark` against Synap RESP3 vs Redis 7 on the same host —
   GET/SET/INCR/LPUSH/LRANGE/SADD, with `-P 1` and pipelined `-P 16`; plus a native SynapRPC
   measurement via the existing bench harness. Results published under `docs/benchmarks/`.
3. Verify RESP3 pipelining depth in `resp3/server.rs` (read-many-before-flush); fix if the
   loop flushes per-command.
4. Cheap, high-leverage wins implemented now: optional allocator feature flag
   (mimalloc, following the old analysis F-011) and parallel-by-shard MGET/MSET in
   `core/kv_store.rs` (old F-007).
5. Ship/defer decision recorded per remaining parity item — BLPOP/BRPOP/BZPOPMIN,
   PSUBSCRIBE + keyspace notifications, HSCAN/SSCAN/ZSCAN cursors, LFU eviction, IO threads.
   Each decision lands in the analysis README and, for deferrals, gets a follow-up rulebook
   task created BEFORE this task archives (deferred-items protocol).

## Impact
- Affected specs: none directly; benchmark methodology documented in docs/benchmarks/
- Affected code: `crates/synap-core/src/kv_store.rs` (MGET/MSET), `crates/synap-server/src/`
  (allocator flag, resp3 server flush loop), `benches/`
- Breaking change: NO (allocator is opt-in feature flag)
- User benefit: credible published Redis comparison for the 1.0 announcement; measurable
  latency/throughput wins on multi-key operations
