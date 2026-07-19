# phase7 perf: RESP3 pipeline flush, honest MGET finding, SynapRPC bench numbers
**Source**: manual
**Date**: 2026-07-09
**Related Task**: phase7_v1-redis-benchmark-and-perf
**Tags**: analysis:synap-vs-redis, phase7, resp3, pipelining, benchmark, mimalloc, honest-measurement
phase7 (Redis benchmark + perf). Environment blocker: no redis-benchmark/redis-server on Windows (Redis isn't native) → live head-to-head deferred to phase9_redis-benchmark-live-run; the code+docs were done.

Key results:
- RESP3 pipelining (1.2): the loop flushed after EVERY command. Fixed with pipeline-aware flush in crates/synap-server/src/protocol/resp3/server.rs: `if reader.buffer().is_empty() { writer.flush().await?; }` after each response, plus flush on clean-EOF. Only defers while more commands are buffered (no deadlock; the next parse consumes from the buffer without awaiting the socket). Raw-socket e2e test added (3 pipelined SETs → 3 ordered +OK).
- MGET/MSET (1.4): ALREADY shard-grouped in-tree (one lock per shard per batch, not per key) — F-007 was resolved before phase7. Added kv_bench mget_vs_sequential. HONEST measured result (uncontended, single thread, release): sequential_1000 ~64µs, mget_1000 ~71µs — grouped is ~10% SLOWER uncontended because bucketing/routing/result-assembly bookkeeping outweighs lock-acquisition savings when locks aren't contended. The grouping wins under CONTENTION. Did NOT ship a false "faster" claim; corrected the analysis annotation to the measured truth and left true cross-core parallelism (rayon) as post-1.0 (dispatch overhead doesn't pay for typical batch sizes).
- Native SynapRPC (1.6): protocol_bench against a live release server. SET+GET round-trip: SynapRPC 168µs < RESP3 234µs < HTTP 478µs (~2.8x HTTP). SET: 80/108/193µs. The isolated GET-only row was anomalous (HTTP fastest) — flagged as a likely bench artifact (HTTP keep-alive vs per-call raw-TCP client overhead), not hidden.
- mimalloc (1.3): opt-in `--features mimalloc` + #[global_allocator] in main.rs; both builds compile (C toolchain present).

Windows gotcha: a lingering synap-server.exe (from an e2e ServerGuard that didn't fully drop) locks target/release/synap-server.exe → `cargo build --release` fails with "Acesso negado (os error 5)". Fix: `taskkill //F //IM synap-server.exe` before the release build. When running protocol_bench against a live server, build with `--no-run` first (no server running), THEN start the server, THEN run the bench (nothing to rebuild → no lock conflict).

Analysis hygiene (1.1): the 2026-04-07 synap-vs-redis analysis claimed CRITICAL "no RESP/no pipelining/no eviction" — all shipped. Added a v1.0 status banner + per-finding status table + ship/defer decision table so implementers stop trusting stale findings.