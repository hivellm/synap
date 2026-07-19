> **Status: shipped.** Benchmark harness, script and docs landed on
> `main`. Artefacts: `synap-server/benches/redis_vs_synap.rs`,
> `scripts/bench-redis-comparison.sh`,
> `docs/benchmarks/redis-vs-synap.md`.

## 1. Dependencies and Feature Gate
- [x] 1.1 `redis = { version = "0.27", features = ["tokio-comp"] }` added to `synap-server/Cargo.toml` dev-dependencies under the `s2s` feature
- [x] 1.2 `reqwest = { version = "0.12", features = ["blocking", "json"] }` added to dev-dependencies under the `s2s` feature
- [x] 1.3 `cargo check --features s2s` compiles cleanly

## 2. Benchmark Helper Script
- [x] 2.1 `scripts/bench-redis-comparison.sh` created — starts Redis (Docker or local binary), launches Synap, runs `cargo bench --bench redis_vs_synap --features s2s`, tees output to `docs/benchmarks/latest-run.txt`, tears everything down
- [x] 2.2 Script is executable and verified on Linux, macOS and Windows (WSL)

## 3. Benchmark Implementation (benches/redis_vs_synap.rs)
- [x] 3.1 Setup: connects to Redis and Synap HTTP, asserts both respond before benching
- [x] 3.2 `set_throughput` group: SET at 64B / 256B / 1KB / 4KB
- [x] 3.3 `get_throughput` group: pre-populates 10K keys and measures GET
- [x] 3.4 `mset_throughput` group: 100-pair batch via Redis `MSET` and Synap `POST /kv/mset`
- [x] 3.5 `incr_throughput` group: INCR counter
- [x] 3.6 `pipeline_throughput` group: Redis pipeline of 50 commands vs Synap MSET equivalent
- [x] 3.7 `bitcount_throughput` group: BITCOUNT on 64KB / 512KB / 1MB bitmaps
- [x] 3.8 `pfadd_pfcount` group: HyperLogLog PFADD + PFCOUNT
- [x] 3.9 `concurrent_reads` group: 8 parallel threads GETting distinct keys
- [x] 3.10 Criterion `BenchmarkId` labels "redis" vs "synap" for side-by-side charts

## 4. Results Documentation
- [x] 4.1 `docs/benchmarks/redis-vs-synap.md` written with hardware requirements, reproduction commands, methodology and results table
- [x] 4.2 Results table filled in with the first measured run
- [x] 4.3 `docs/analysis/synap-vs-redis/execution-plan.md` Phase 3.4 updated with the measured numbers

## 5. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 5.1 Connection smoke test asserts Redis and Synap both respond under `#[cfg(feature = "s2s")]`; no-op when the feature is absent
- [x] 5.2 `cargo test --features s2s -- redis_comparison` passes when both servers are running
- [x] 5.3 `cargo test` without `s2s` still passes — the bench file compiles cleanly under both configurations
- [x] 5.4 Update or create documentation covering the implementation (`docs/benchmarks/redis-vs-synap.md`)
- [x] 5.5 Write tests covering the new behavior (smoke test in 5.1 + bench harness assertions)
- [x] 5.6 Run tests and confirm they pass
