# IO Threads vs. the Sharded Async Model — Evaluation

**Status:** Complete — decision: do **not** implement Redis-style IO threads.
**Backlog item:** `phase9_redis-parity-feature-backlog` 1.5
**Date:** 2026-07-10

## Question

Redis 6 added *IO threads* (`io-threads N`) to offload socket read, protocol
parsing, and socket write to helper threads while command execution stays on the
single main thread. Should Synap add an equivalent? The backlog item explicitly
says **measure against the existing sharded model first**.

## Why Redis needs IO threads (and Synap's starting point differs)

Redis executes all commands on **one** thread. Under many connections the single
thread saturates on `read()`/parse/`write()` syscalls before the command logic
itself is the bottleneck. IO threads are a targeted workaround: parallelize *only*
the I/O around a still-serial execution core.

Synap does not share that constraint:

- **Async I/O on a multi-threaded runtime.** Networking runs on Tokio's
  work-stealing scheduler, which already multiplexes socket read/parse/write
  across all worker threads (one per core by default). There is no single I/O
  thread to relieve.
- **64-way sharded stores.** Every datatype store (`KVStore`, `HashStore`,
  `ListStore`, `SetStore`, `SortedSetStore`) partitions keys across 64 shards,
  each behind its own `RwLock`. Command *execution* is therefore already
  concurrent across cores — the very thing Redis keeps serial.

So the two layers Redis parallelizes separately (I/O via IO threads, execution
never) are *both* already parallel in Synap.

## Measurement

`cargo bench -p synap-server --bench kv_bench -- concurrent_operations` runs
`N` concurrent tasks × 100 ops each against a shared store, for `N ∈ {1,4,16,64}`.
Per-op latency (lower is better) and derived throughput:

| Concurrency | SET µs/batch | SET ns/op | SET Mops/s | GET µs/batch | GET ns/op | GET Mops/s |
|------------:|-------------:|----------:|-----------:|-------------:|----------:|-----------:|
| 1           | 46.9         | 469       | 2.13       | 31.0         | 310       | 3.23       |
| 4           | 133.0        | 332       | 3.01       | 64.4         | 161       | 6.21       |
| 16          | 236.2        | 147       | 6.78       | 128.8        | 80.5      | 12.4       |
| 64          | 595.0        | 93        | 10.8       | 319.0        | 49.8      | 20.1       |

(Machine: the CI/dev host; absolute numbers vary but the *scaling* is the point.)

**Per-op throughput scales ~5.0× (SET) and ~6.2× (GET) from 1 → 64 concurrent
tasks.** Latency per operation falls monotonically as concurrency rises, i.e. the
sharded store absorbs parallel load across cores rather than serializing it. That
is exactly the scaling IO threads are meant to unlock on Redis — Synap already has
it, at the execution layer *and* (via Tokio) the I/O layer.

## Conclusion

Adding an explicit IO-thread pool would sit on top of Tokio's existing I/O
threads and double-schedule the same work — extra hand-off latency and
synchronization for no parallelism that the runtime + sharding don't already
provide. IO threads solve a single-threaded-execution problem Synap does not
have.

**Decision: do not implement IO threads.** The sharded async model is the
architectural equivalent and measurably scales with concurrency. If a future
profile ever shows syscall overhead dominating (e.g. huge fan-out of tiny
requests), the levers to reach for first are Tokio worker-thread count,
`TCP_NODELAY`/batching at the framing layer, and shard-count tuning — not a
bolt-on IO-thread pool.

## Reproduce

```bash
cargo bench -p synap-server --bench kv_bench -- concurrent_operations
```
