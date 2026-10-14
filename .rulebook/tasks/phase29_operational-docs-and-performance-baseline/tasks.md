## 1. Implementation
- [x] 1.1 Add a benchmark script recording p50/p95/p99 latency, throughput, memory, workload, durability mode, build and hardware per run — `scripts/bench/perf_baseline.py` (Python driver: fresh server per cell, 4 durability modes x RESP3/SynapRPC x get/set/list/set-pipelined, N interleaved runs, median + spread, JSON + Markdown summary, `--server-bin` for other binaries with the rustc commit read from the binary, a cell the binary cannot run fails alone with its reason) + `crates/synap-server/examples/bench_loadgen.rs` (raw-socket Rust load generator, exact percentiles, sysinfo RSS sampling and host facts); documented in `scripts/bench/README.md`
- [x] 1.2 Add a scheduled and manually triggered benchmark workflow for a controlled runner, with no latency gate on shared runners — `.github/workflows/perf-baseline.yml`: `baseline` job on `vars.PERF_BENCH_RUNNER` (labeled dedicated runner of any OS, steps in bash, comparable), `smoke` job on `ubuntu-latest` when the variable is unset (1 short run, labeled not comparable); pinned toolchain, `--locked`, JSON artifact, no threshold; documented in `docs/development/CI_GATES.md#performance-baseline`
- [ ] 1.3 Record the 1.4.0 baseline and a 1.3.2 comparison from repeated runs
- [ ] 1.4 Write the 1.4.0 guarantees document: durability per fsync mode, authorization per transport, Hub mode, limits, and error replies versus in-memory state
- [ ] 1.5 Label historical analyses under `docs/analysis/` with the revision they describe and link the current architecture

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
