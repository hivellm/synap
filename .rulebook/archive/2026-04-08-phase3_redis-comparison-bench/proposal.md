# Proposal: Redis Comparison Benchmarks — Real Redis vs Synap Side-by-Side

## Why

Current benchmarks measure Synap in isolation and reference Redis performance from published
numbers (e.g. "Redis HSET baseline ~100K ops/sec"). This is not a real comparison: published
numbers vary by hardware, Redis version, and workload shape; they cannot be trusted as a
baseline for hardware-specific decisions or performance claims.

To make honest, reproducible performance claims ("Synap is X% faster/slower than Redis on
SET/GET/BITCOUNT/PFADD on this hardware"), we need benchmarks that:
1. Spin up both a real Redis instance (via Docker or local binary) and a Synap server
2. Run identical workloads against both using the same Criterion harness
3. Use the RESP protocol for Redis (via the `redis` crate) and HTTP for Synap
4. Produce a side-by-side report with the same timing methodology

This is a mandatory prerequisite for the Phase 3 goal ("Published benchmark shows Synap
>=1.5x Redis on 8-core multi-client SET workload", execution-plan.md Phase 3.4).

## What Changes

- ADDED: `synap-server/benches/redis_vs_synap.rs` — S2S comparison benchmark
  - Connects to Redis on 127.0.0.1:6379 and Synap HTTP on 127.0.0.1:15500
  - Benchmarks: SET, GET, MSET (100 keys), INCR, DEL, EXPIRE, BITCOUNT, PFADD/PFCOUNT
  - Uses Criterion with `--features s2s` gate so it never runs in standard CI
  - Produces wall-clock throughput (ops/sec) and latency (p50/p99) for each operation
- ADDED: `scripts/bench-redis-comparison.sh` — helper script that starts Redis via Docker,
  starts Synap, runs the benchmark, stops both, prints the comparison table
- ADDED: `docs/benchmarks/redis-vs-synap.md` — benchmark methodology, hardware requirements,
  how to reproduce, and template for recording results
- MODIFIED: `synap-server/Cargo.toml` — add `redis` crate as dev-dependency (RESP client),
  add `reqwest` blocking as dev-dependency for Synap HTTP calls in benchmarks
- MODIFIED: `synap-server/Cargo.toml` — `s2s` feature already exists; document it covers
  both integration tests and S2S benchmarks

## Impact

- Affected specs: specs/bench/spec.md
- Affected code: synap-server/benches/redis_vs_synap.rs (new); scripts/bench-redis-comparison.sh (new)
- Breaking change: NO (gated behind --features s2s; never runs in default CI)
- User benefit: Honest, reproducible, hardware-specific comparison against Redis 7; clear
  evidence for performance claims; identifies which operations need more work
