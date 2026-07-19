# 3. No Redis-style IO threads — sharded async model is the equivalent

**Status**: proposed
**Date**: 2026-07-10

## Context

phase9_redis-parity-feature-backlog item 1.5 asked to evaluate/implement Redis 6 IO threads, measuring against the existing sharded model first. Redis IO threads offload socket read/parse/write from its single command-execution thread. Synap already runs networking on Tokio's multi-threaded work-stealing runtime (I/O parallel across cores) AND executes commands against 64-way sharded per-datatype stores (execution parallel across cores).

## Decision

Do NOT implement IO threads. The concurrent_operations benchmark shows per-op throughput scaling ~5.0x (SET) and ~6.2x (GET) from 1 to 64 concurrent tasks — per-op latency falls monotonically as concurrency rises, proving the sharded async model already parallelizes load across cores. An explicit IO-thread pool would sit atop Tokio's existing I/O threads and double-schedule the same work for no additional parallelism. Full evaluation in docs/analysis/io-threads-evaluation.md. Future levers if syscall overhead ever dominates: Tokio worker count, framing-layer batching/TCP_NODELAY, shard-count tuning.

## Consequences

IO threads are closed as "evaluated, not needed". No code change. The evaluation doc + this ADR are the deliverable for backlog item 1.5.
