# Proposal: phase29_operational-docs-and-performance-baseline

Source: docs/analysis/next-release-review-2026-10-12/ (separate improvement proposals in ecosystem.md and README.md)
Target release: 1.4.0

## Why

- **Performance**: CI only compiles the benchmarks (`cargo bench --no-run` in
  `.github/workflows/rust-ci.yml`). There is no recorded baseline, so 1.4.0's
  extra durability work (native writes now hitting the WAL, the snapshot
  barrier, write ordering) cannot be measured against 1.3.2. Raw numbers taken
  on shared runners are too noisy to gate on.
- **Documentation**: 1.4.0 changes what the server guarantees: authorization
  per transport, Hub mode policy, durability per fsync mode, request limits.
  Those guarantees need one current description. Historical analyses under
  `docs/analysis/` reference code that has since moved, such as the dissolved
  `synap-protocol` crate in `redis-parity-deep-dive.md` and the former
  monolithic handler path in `synap-vs-redis/set-deep-dive.md`, without saying
  which revision they describe.

## What Changes

- A benchmark script records p50/p95/p99 latency, throughput, memory,
  workload, durability mode, build and hardware for each run.
- A scheduled and manually triggered workflow runs it on a controlled runner.
  No latency gate runs on shared runners.
- The 1.4.0 baseline comes from repeated runs, with 1.3.2 measured the same way
  for comparison.
- A guarantees document describes the verified 1.4.0 behavior: durability per
  fsync mode, authorization per transport, Hub mode, limits, and what an error
  reply means for in-memory state.
- Historical analyses are labeled with the revision they describe and link the
  current architecture.

## Impact

- Affected specs: docs (guarantees), bench (baseline)
- Affected code: `scripts/bench/`, `.github/workflows/`, `docs/`
- Dependencies: phase15 through phase28, whose final behavior the docs and the
  baseline describe
- Breaking change: NO
- User benefit: operators can read what the server actually guarantees, and
  performance changes are measured.
