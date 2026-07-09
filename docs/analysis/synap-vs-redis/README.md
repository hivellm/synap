# Synap vs Redis — Competitive Analysis

**Date**: 2026-04-07
**Scope**: Synap in-memory KV/cache subsystem compared to Redis on performance, features, and reliability.
**Goal**: Identify actionable gaps to make Synap competitive with Redis.
**User-reported critical pain point**: KV `SET` path — see [set-deep-dive.md](set-deep-dive.md).

> ## ⚠️ Status update — v1.0 (2026-07-09), phase7
>
> **This 2026-04-07 analysis is largely superseded. Do not treat its "CRITICAL"
> blockers as open.** As of v1.0 the three headline blockers are shipped:
>
> - **RESP binary protocol (F-001) — ✅ RESOLVED.** RESP3 listener (94-command
>   dispatch) on 6379 + native SynapRPC/MessagePack on 15501.
> - **Pipelining (F-002) — ✅ RESOLVED.** RESP3 server uses pipeline-aware
>   flushing (batched syscall per pipeline, not per command).
> - **Eviction (F-003) — ✅ RESOLVED.** Six Redis eviction policies.
> - **Write lock on GET (F-004) — ✅ RESOLVED.** `AtomicU32` LRU fast path.
>
> **Still open**, each with a decision recorded in this phase:
> - Memory-accounting drift (F-006) → tracked in phase6g (memory accounting).
> - MGET/MSET (F-007) → shard-grouped (one lock per shard), benchmarked in phase7; true cross-core parallelism deferred.
> - Allocator (part of F-011) → opt-in `mimalloc` feature flag in phase7.
> - Blocking ops / PSUBSCRIBE / SCAN cursors / IO threads (F-008/9/10/F-011) →
>   ship/defer decisions recorded below and in per-item follow-up tasks.
>
> Per-finding status is annotated on each finding header in [findings.md](findings.md).
> The benchmark methodology and results live in [docs/benchmarks/](../../benchmarks/).

## Executive Summary

Synap is **~2 years ahead** of Redis on the concurrency model (64-way sharded `parking_lot::RwLock` vs Redis single-threaded event loop) and **~20-30% more memory-efficient per entry** thanks to the compact `StoredValue` enum and adaptive HashMap→RadixTrie storage. However, it is **1-2 years behind** on the wire protocol: every operation goes through HTTP+JSON, with ~1-2ms overhead per request vs ~0.2ms for Redis RESP. Command coverage is ~140-150 vs Redis ~200 (~75% functional parity).

**Three critical problems block direct competitiveness:**

1. **No binary protocol (RESP)** — HTTP/JSON overhead nullifies the architectural advantages
2. **Eviction is not implemented** — `max_memory_mb` only returns an error; LFU/TTL declared in the enum but no logic exists
3. **The SET hot path has ~20 distinct correctness, performance, and feature problems** (see deep dive). This is the user's #1 reported pain point in real projects.

A separate but related issue: **memory accounting drifts** because the SET overwrite path never subtracts the old value's size, so reported memory usage becomes meaningless after a few overwrites.

## Documents

- [findings.md](findings.md) — 12 numbered findings (F-001..F-012) with file:line evidence, impact, priority, and effort
- [set-deep-dive.md](set-deep-dive.md) — Detailed review of every issue in the KV SET path (the user's #1 pain point)
- [execution-plan.md](execution-plan.md) — 3-phase improvement plan (~12-15 weeks)

## Synap Strengths

| Aspect | Synap | Redis |
|---|---|---|
| Concurrency model | 64 shards, `parking_lot::RwLock` — true parallel reads | Single-threaded event loop |
| KV storage | Adaptive (HashMap <10K, RadixTrie ≥10K) | Global hashtable + expires table |
| Memory/entry | ~24-32 bytes (compact enum) | ~50 bytes |
| Cluster | Native (hash slots + RAFT) | Optional Cluster mode |
| Protocols | REST, WS, MCP, UMICP | RESP, RESP3 |
| Tests | 620 functions across 51 modules | Mature suite |

## Synap Weaknesses

| Aspect | Gap | Severity |
|---|---|---|
| Wire protocol | No RESP, only HTTP+JSON | CRITICAL |
| Pipelining | Not implemented | CRITICAL |
| Eviction | Not implemented (only returns error) | CRITICAL |
| **SET path** | **~20 distinct issues — see deep-dive** | **CRITICAL** |
| LRU update path | Acquires write lock on GET | HIGH |
| `handlers.rs` | 11,595 lines in one file | HIGH (maintainability) |
| Memory stats | Drift (incremented on insert, never decremented on overwrite/delete) | HIGH |
| Blocking ops | No BLPOP/BRPOP/BZPOPMIN | MEDIUM |
| Pub/Sub patterns | No PSUBSCRIBE | MEDIUM |
| MGET/MSET | Sequential across shards | MEDIUM |
| Cursor SCAN | Only on KV | MEDIUM |
| Streams | XPENDING/XAUTOCLAIM incomplete | MEDIUM |
| Allocator | Default Rust (no mimalloc/jemalloc) | MEDIUM |
| IO threads | Not implemented | MEDIUM |

## Ship / defer decisions for 1.0 (phase7)

Each remaining Redis-parity gap has an explicit decision. Shipped items link to
evidence; deferred items link to a follow-up task created before phase7 archived.

| Item | Decision | Rationale |
|---|---|---|
| RESP3 + pipelining (F-001/F-002) | **Shipped 1.0** | RESP3 listener + pipeline-aware flush |
| Eviction, 6 policies (F-003) | **Shipped 1.0** | approximated-LRU sampling |
| Lock-free GET (F-004) | **Shipped 1.0** | `AtomicU32` LRU |
| Shard-grouped MGET/MSET (F-007) | **Shipped 1.0** (partial) | one lock per shard, not per key; uncontended latency ~parity (measured, `kv_bench mget_vs_sequential`); true cross-core parallelism post-1.0 |
| Opt-in `mimalloc` allocator (F-011a) | **Shipped 1.0** | `--features mimalloc` |
| Memory-accounting drift (F-006) | **Deferred** | large cross-datatype refactor → phase6g |
| Blocking ops BLPOP/BRPOP/BZPOPMIN (F-008) | **Post-1.0** | needs a client-wait/notify mechanism; clients can poll meanwhile |
| PSUBSCRIBE + keyspace notifications (F-009) | **Post-1.0** | additive pattern-sub + event feature |
| HSCAN/SSCAN/ZSCAN cursors (F-010) | **Post-1.0** | KV SCAN ships; collection cursors additive |
| LFU eviction | **Post-1.0** | 6 policies ship; LFU counter-decay additive |
| IO threads (F-011b) | **Post-1.0** | 64-shard model already parallelizes; low marginal gain |

Deferred parity items are tracked in follow-up rulebook tasks (see the phase7
task closure). No item is left as an undocumented gap.

## Competitive Position

To compete head-to-head, the absolute priority is **Phase 1** of the execution plan: fix the SET path, implement real eviction, add RESP+pipelining, and remove the write lock from GET. Those four items close the main latency gap and unlock real cache use cases.

After Phase 1, Synap has a real chance to **outperform Redis on multi-core workloads** thanks to native sharding — something Redis only achieves via Cluster mode with extra operational complexity.
