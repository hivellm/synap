## 1. Implementation
- [x] 1.1 Annotate docs/analysis/synap-vs-redis/ findings with current status (banner + per-finding table; commit d8c8484)
- [x] 1.2 Verify RESP3 pipelining depth: was flushing per command → pipeline-aware flush + e2e test (commit 7bebd2f, 9/9 e2e)
- [x] 1.3 Opt-in mimalloc allocator feature flag; both builds verified (commit 8832381; A/B bench tracked in phase9_redis-benchmark-live-run)
- [x] 1.4 MGET/MSET are shard-grouped (one lock per shard); added kv_bench mget_vs_sequential — measured honestly (uncontended ~parity; true cross-core parallelism tracked in phase9) (commit 316c4e1)
- [x] 1.5 redis-benchmark methodology + script documented; live execution tracked in phase9_redis-benchmark-live-run (no Redis tooling on Windows) (commit d8c8484)
- [x] 1.6 Native SynapRPC path measured via protocol_bench against a live server: SET+GET 168µs (SynapRPC) vs 234µs (RESP3) vs 478µs (HTTP) (commit b1c2b54)
- [x] 1.7 Published methodology + native-protocol results under docs/benchmarks/redis-vs-synap.md (live Redis tables tracked in phase9)
- [x] 1.8 Ship/defer decision table per parity item; follow-up tasks created: phase9_redis-parity-feature-backlog, phase9_redis-benchmark-live-run (commits d8c8484, b1c2b54)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (analysis annotation, benchmark methodology+results, CHANGELOG)
- [x] 2.2 Write tests covering the new behavior (RESP3 pipelining e2e test; kv_bench mget_vs_sequential)
- [x] 2.3 Run tests and confirm they pass (full workspace suite: 1710 passed, 0 failed; live e2e incl. pipelining test 9/9)
