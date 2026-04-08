## 1. Dependencies and Feature Gate
- [ ] 1.1 Add `redis = { version = "0.27", features = ["tokio-comp"] }` to `synap-server/Cargo.toml` dev-dependencies, gated under `s2s` feature
- [ ] 1.2 Add `reqwest = { version = "0.12", features = ["blocking", "json"] }` to dev-dependencies, gated under `s2s` feature
- [ ] 1.3 Confirm `cargo check --features s2s` compiles cleanly

## 2. Benchmark Helper Script
- [ ] 2.1 Create `scripts/bench-redis-comparison.sh`:
  - Check if Docker is available; if yes, `docker run -d --name synap-bench-redis -p 6379:6379 redis:7-alpine`
  - If no Docker, check if `redis-server` binary is on PATH and start it
  - Start Synap server in background: `cargo run --release -- --config config.yml`
  - Wait for both to be ready via health check loop
  - Run `cargo bench --bench redis_vs_synap --features s2s 2>&1 | tee docs/benchmarks/latest-run.txt`
  - Stop Redis container and Synap, print summary table
- [ ] 2.2 Make the script executable and verify it runs on Linux, macOS, and Windows (WSL)

## 3. Benchmark Implementation (benches/redis_vs_synap.rs)
- [ ] 3.1 Setup: connect to Redis (`redis::Client::open("redis://127.0.0.1:6379")`) and Synap HTTP (`reqwest::blocking::Client`); assert both respond before proceeding
- [ ] 3.2 Benchmark group `set_throughput`: SET of 64B, 256B, 1KB, 4KB values — Redis via `SET key value` (RESP), Synap via `POST /kv/{key}` (HTTP)
- [ ] 3.3 Benchmark group `get_throughput`: pre-populate 10K keys, Criterion measures GET on both Redis and Synap
- [ ] 3.4 Benchmark group `mset_throughput`: batch of 100 key-value pairs via Redis `MSET` and Synap `POST /kv/mset`
- [ ] 3.5 Benchmark group `incr_throughput`: INCR on a counter key
- [ ] 3.6 Benchmark group `pipeline_throughput`: Redis pipelining 50 commands vs Synap MSET equivalent
- [ ] 3.7 Benchmark group `bitcount_throughput`: BITCOUNT on 64KB, 512KB, 1MB bitmaps
- [ ] 3.8 Benchmark group `pfadd_pfcount`: HyperLogLog PFADD then PFCOUNT
- [ ] 3.9 Benchmark group `concurrent_reads`: 8 Rayon threads simultaneously GETting distinct keys — measures read parallelism advantage of Synap (multi-threaded) vs Redis (single-threaded event loop)
- [ ] 3.10 Use `criterion::BenchmarkId` labeling "redis" vs "synap" so Criterion generates side-by-side comparison charts

## 4. Results Documentation
- [ ] 4.1 Create `docs/benchmarks/redis-vs-synap.md` with hardware requirements, reproduction commands, methodology, and results table template
- [ ] 4.2 After first successful run: fill in the results table with measured numbers from this machine
- [ ] 4.3 Update `docs/analysis/synap-vs-redis/execution-plan.md` Phase 3.4 with real measured numbers

## 5. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 5.1 Write connection smoke test: assert Redis and Synap both respond when `#[cfg(feature = "s2s")]` is active; test is a no-op (passes immediately) when feature is absent
- [ ] 5.2 Confirm `cargo test --features s2s -- redis_comparison` passes when both servers are running
- [ ] 5.3 Confirm `cargo test` (without s2s) still passes — benchmark file must compile cleanly under both configurations
- [ ] 5.4 Update or create documentation covering the implementation
- [ ] 5.5 Write tests covering the new behavior
- [ ] 5.6 Run tests and confirm they pass
