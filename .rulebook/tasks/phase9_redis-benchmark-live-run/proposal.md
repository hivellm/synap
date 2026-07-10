# Proposal: phase9_redis-benchmark-live-run

Source: docs/benchmarks/redis-vs-synap.md (v1.0 methodology, phase7)

## Why
phase7 documented the v1.0 redis-benchmark methodology but could not execute the
live head-to-head: the phase7 environment was Windows with no `redis-benchmark`/
`redis-server` (Redis is not native to Windows). The published 1.0 Redis
comparison is still owed and must be produced in a Redis-equipped environment.

## What Changes
Run the already-documented methodology on a host with Redis 7 + redis-benchmark
(WSL/Docker/Linux), Synap built `--release`:
1. RESP3 vs Redis 7 via `redis-benchmark` (GET/SET/INCR/LPUSH/LRANGE/SADD), `-P 1` and `-P 16`.
2. Native SynapRPC path via `cargo bench --bench protocol_bench`.
3. mimalloc A/B: `kv_bench` with and without `--features mimalloc`.
4. Replace the stale 0.9.0/HTTP tables in docs/benchmarks/redis-vs-synap.md with the fresh numbers.

## Impact
- Affected specs: benchmark methodology already documented in docs/benchmarks/
- Affected code: none (measurement + docs only)
- Breaking change: NO
- User benefit: credible, current published Redis comparison for the 1.0 story
