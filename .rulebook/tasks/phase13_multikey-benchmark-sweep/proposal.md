# Proposal: phase13_multikey-benchmark-sweep

Source: docs/analysis/redis-parity-deep-dive.md (plan, "definition of beating Redis")

## Why

Every number published so far uses `redis-benchmark`'s default single-key shape
— Synap's **worst case** (one hot key defeats the 64-way sharding and multi-core
execution entirely) and Redis's best case (a single thread with a perfectly hot
cache line). The story is incomplete without the workload where the
architectures actually differ: **many keys, many cores**. `redis-benchmark -r N`
randomizes keys across a keyspace, letting Synap's shards run in parallel while
Redis stays serial on one thread.

## What Changes

1. Run the full sweep with `-r 1000000` (randomized keyspace) at `-P 1` and
   `-P 16`, plus a high-connection variant (`-c 200`), Synap vs Redis 7, same
   Docker network/method as the existing comparison.
2. Also run the native SynapRPC load-gen with randomized keys (extend
   `synap_bench` with a `-r`-style keyspace option).
3. Publish the results as a new section in `docs/benchmarks/redis-vs-synap.md`
   ("multi-key: where the architectures diverge"), alongside the single-key
   tables, with an honest read of both.
4. If multi-key does NOT show the expected Synap advantage, profile and file the
   findings as follow-up tasks (that result would indicate a scalability defect
   worth fixing more than any single-key micro-optimization).

## Impact

- Affected specs: none (measurement + docs; small synap_bench extension)
- Affected code: crates/synap-server/src/bin/synap_bench.rs (keyspace option)
- Breaking change: NO
- User benefit: credible published evidence of Synap's multi-core scaling vs
  Redis — the headline claim for choosing Synap
