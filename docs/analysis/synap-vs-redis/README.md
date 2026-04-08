# Synap vs Redis — Competitive Analysis

**Date**: 2026-04-07
**Scope**: Synap in-memory KV/cache subsystem compared to Redis on performance, features, and reliability.
**Goal**: Identify actionable gaps to make Synap competitive with Redis.
**User-reported critical pain point**: KV `SET` path — see [set-deep-dive.md](set-deep-dive.md).

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

## Competitive Position

To compete head-to-head, the absolute priority is **Phase 1** of the execution plan: fix the SET path, implement real eviction, add RESP+pipelining, and remove the write lock from GET. Those four items close the main latency gap and unlock real cache use cases.

After Phase 1, Synap has a real chance to **outperform Redis on multi-core workloads** thanks to native sharding — something Redis only achieves via Cluster mode with extra operational complexity.
