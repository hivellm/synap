# Proposal: phase9_redis-parity-feature-backlog

Source: docs/analysis/synap-vs-redis/ (ship/defer decisions, phase7)

## Why
phase7 recorded explicit post-1.0 decisions for the remaining Redis-parity gaps.
Each deferred item needs a tracked home so it is not lost. This task is that
home: the backlog of Redis-parity features intentionally shipped after 1.0
because each is additive (not a correctness/stability blocker) and non-trivial.

## What Changes
Implement, each as its own increment with tests:
1. Blocking list/zset ops: BLPOP / BRPOP / BZPOPMIN (client-wait + notify).
2. Pattern pub/sub PSUBSCRIBE + keyspace notifications.
3. Collection cursors: HSCAN / SSCAN / ZSCAN (KV SCAN already ships).
4. LFU eviction policy (counter with decay) alongside the existing 6 policies.
5. Evaluate IO threads (the 64-shard model already parallelizes; measure before building).

## Impact
- Affected specs: pub/sub, list/zset, set/hash/zset SCAN, eviction (new requirements)
- Affected code: `crates/synap-core/src/core/*`, `crates/synap-server/src/protocol/*`
- Breaking change: NO (all additive)
- User benefit: closes the remaining Redis feature-parity gap for 1.x
