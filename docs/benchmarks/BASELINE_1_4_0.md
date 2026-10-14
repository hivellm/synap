# Performance Baseline: 1.4.0 vs 1.3.2

Recorded 2026-10-14 with [`scripts/bench/perf_baseline.py`](../../scripts/bench/README.md#performance-baseline).
The raw reports, every run included, are
[`reports/perf-baseline-1.4.0.json`](reports/perf-baseline-1.4.0.json) and
[`reports/perf-baseline-1.3.2.json`](reports/perf-baseline-1.3.2.json).

## Summary

- **Persistence off:** 1.4.0 runs as fast as 1.3.2. Every cell is within
  -5.1% to +3.6% of 1.3.2, inside the run-to-run spread (4% to 20%).
- **Reads** (`get`) are unchanged in every durability mode.
- **Writes with the WAL on** are slower in 1.4.0, because 1.4.0 logs them
  and 1.3.2 did not. 1.3.2 did not log a plain RESP3 or SynapRPC write at
  all; it logged only MULTI/EXEC transactions. Its `never`, `periodic` and
  `always` write numbers are therefore its `off` numbers, and a crash lost
  those writes. Most of the drop is the cost of the durability fix:
  - one write in flight per client: -30% to -37% in `never` and `periodic`,
    and about 1,200 ops/s in `always`, where every group commit waits for
    an fsync of about 1.4 ms on this disk;
  - SynapRPC pipelined: -47% to -50% in `never`/`periodic`, 15,275 ops/s
    in `always`. A pipelined batch is committed as a group.
- **One part can be avoided:** pipelined RESP3 writes with the WAL on
  (-89% in `never`/`periodic`, -99.7% in `always`). A RESP3 connection
  waits for each write's WAL acknowledgement before it reads the next
  command, so a pipelined batch of 16 makes 16 group commits instead of one.
  A second, smaller avoidable cost is in the WAL writer: about half of each
  acknowledgement is spent passing the write to tokio's blocking thread pool
  and back. [Why writes with the WAL on are slower](#why-writes-with-the-wal-on-are-slower)
  explains both, with the evidence and a proposed fix for each.

## How it was measured

| | |
|---|---|
| Machine | AMD Ryzen 9 7950X3D (16 cores, 32 threads), 127 GiB RAM, Windows 11 Pro (build 26200), x86_64 |
| WAL drive | the system temp directory, on a Samsung SSD 970 PRO 1TB (NVMe) |
| Load | the load generator runs on the same machine and uses loopback TCP |
| 1.4.0 server | built at `e687427`, a merge of `release/v1.4.0` at `295faaa`, with `cargo build --release --locked -p synap-server` (fat LTO, one codegen unit) and rustc 1.99.0-nightly (6f72b5dd5 2026-07-22). The report names `1cacdfc`, the commit checked out during the runs. That commit adds only the script's `--merge`, its tests, and the CI and docs lines about them; it changes no server code. |
| 1.3.2 server | the 1.3.2 release build (`release/v1.3.2` as merged into `main`, checked out at `bfecd0c`), profile `release`, built by the same rustc commit (read from the binary) |
| Load generator | `crates/synap-server/examples/bench_loadgen.rs`, release |
| Runner class | `local`: compare these numbers only with runs on this machine |

The script ran with its defaults: 4 clients, 5 s measured after 1 s of
warmup, a keyspace of 100,000 keys, 64-byte values, and a pipeline depth of
16 for `set-pipelined`. Each cell starts a fresh server in its own scratch
directory, and the WAL is written there. Each durability mode uses the
persistence settings in `scripts/bench/bench-config.yml` and changes only
`enabled` and `fsync_mode`. That gives a 64 KiB WAL buffer,
`fsync_interval_ms: 1000`, and snapshots every 300 s or 10,000 operations.
The snapshot task checks those triggers once a minute, and almost every
cell's server exits before the first check that could fire, 60 s after
start. One cell is the exception: RESP3 `get` under `always`. Before the
measurement, the load generator writes all 100,000 keys on one connection,
and in `always` mode each of those writes waits for its own fsync. That takes
more than two minutes, so a snapshot may run during that cell, most likely
while the keys are being written. The cell's throughput matches 1.3.2's, so
no snapshot shows in the result.

The two binaries were run in turns, one full matrix at a time: 1.4.0,
1.3.2, 1.4.0, and so on, five times each. Nothing else ran on the machine.
The five single-run reports of each binary were then merged with `--merge`:

```bash
for i in 1 2 3 4 5; do
  python scripts/bench/perf_baseline.py --no-build --runs 1 --out-dir out/new
  python scripts/bench/perf_baseline.py --no-build --runs 1 --out-dir out/old \
      --server-bin <1.3.2>/target/release/synap-server.exe \
      --server-git-sha bfecd0c0c98d353b5d06570c66fb19b1128d3444 --server-profile release
done
python scripts/bench/perf_baseline.py --merge out/new/*.json --out-dir out
python scripts/bench/perf_baseline.py --merge out/old/*.json --out-dir out
```

Each value is the median of a cell's five runs. The spread in parentheses is
(max - min) / median for throughput. Latency is per request, from writing
its batch to reading its reply. Peak RSS is the server's.

## Results

### Durability `off` (no WAL, no snapshots)

| protocol | workload | 1.4.0 ops/s (spread) | 1.3.2 ops/s (spread) | ops/s change | p50 µs 1.4.0 / 1.3.2 | p99 µs 1.4.0 / 1.3.2 | peak RSS MiB 1.4.0 / 1.3.2 |
|---|---|---:|---:|---:|---:|---:|---:|
| resp3 | get | 36,910 (7.8%) | 35,850 (4.9%) | +3.0% | 102 / 105 | 206 / 213 | 34.3 / 34.0 |
| resp3 | set | 34,744 (16.0%) | 35,223 (8.5%) | -1.4% | 108 / 107 | 218 / 212 | 33.6 / 33.8 |
| resp3 | list | 35,701 (20.5%) | 35,542 (4.0%) | +0.4% | 105 / 106 | 214 / 211 | 14.9 / 14.8 |
| resp3 | set-pipelined | 383,737 (14.5%) | 378,965 (17.7%) | +1.3% | 152 / 155 | 296 / 305 | 35.3 / 36.2 |
| synaprpc | get | 34,308 (11.7%) | 33,104 (5.8%) | +3.6% | 108 / 112 | 216 / 224 | 33.9 / 34.0 |
| synaprpc | set | 31,965 (14.0%) | 31,788 (9.3%) | +0.6% | 115 / 115 | 233 / 235 | 33.4 / 33.5 |
| synaprpc | list | 32,564 (14.9%) | 33,714 (6.8%) | -3.4% | 114 / 110 | 226 / 221 | 15.0 / 15.0 |
| synaprpc | set-pipelined | 319,507 (15.2%) | 336,557 (5.3%) | -5.1% | 173 / 164 | 328 / 315 | 35.7 / 35.7 |

### Durability `never` (WAL, no fsync)

| protocol | workload | 1.4.0 ops/s (spread) | 1.3.2 ops/s (spread) | ops/s change | p50 µs 1.4.0 / 1.3.2 | p99 µs 1.4.0 / 1.3.2 | peak RSS MiB 1.4.0 / 1.3.2 |
|---|---|---:|---:|---:|---:|---:|---:|
| resp3 | get | 34,773 (5.5%) | 35,335 (5.7%) | -1.6% | 108 / 106 | 218 / 213 | 34.1 / 34.0 |
| resp3 | set | 23,587 (14.8%) | 35,571 (6.5%) | -33.7% | 160 / 105 | 311 / 216 | 33.5 / 33.7 |
| resp3 | list | 22,838 (15.1%) | 35,489 (8.8%) | -35.6% | 164 / 106 | 324 / 214 | 15.5 / 15.1 |
| resp3 | set-pipelined | 41,332 (23.6%) | 388,981 (10.3%) | -89.4% | 1510 / 150 | 2148 / 291 | 34.7 / 35.7 |
| synaprpc | get | 34,117 (11.4%) | 35,046 (6.4%) | -2.7% | 109 / 107 | 215 / 216 | 35.7 / 34.1 |
| synaprpc | set | 23,063 (7.1%) | 33,031 (14.7%) | -30.2% | 164 / 111 | 292 / 224 | 33.5 / 33.7 |
| synaprpc | list | 22,912 (8.4%) | 33,619 (5.5%) | -31.8% | 165 / 110 | 301 / 219 | 15.6 / 15.1 |
| synaprpc | set-pipelined | 176,881 (17.4%) | 333,766 (8.1%) | -47.0% | 290 / 165 | 560 / 314 | 36.8 / 35.8 |

### Durability `periodic` (WAL, fsync every 1000 ms)

| protocol | workload | 1.4.0 ops/s (spread) | 1.3.2 ops/s (spread) | ops/s change | p50 µs 1.4.0 / 1.3.2 | p99 µs 1.4.0 / 1.3.2 | peak RSS MiB 1.4.0 / 1.3.2 |
|---|---|---:|---:|---:|---:|---:|---:|
| resp3 | get | 35,195 (6.1%) | 36,589 (7.3%) | -3.8% | 107 / 103 | 219 / 206 | 34.2 / 33.9 |
| resp3 | set | 23,733 (8.0%) | 35,761 (6.3%) | -33.6% | 156 / 105 | 309 / 214 | 33.6 / 34.0 |
| resp3 | list | 23,083 (12.5%) | 36,625 (11.2%) | -37.0% | 162 / 103 | 326 / 206 | 15.5 / 15.0 |
| resp3 | set-pipelined | 42,876 (17.1%) | 382,959 (6.0%) | -88.8% | 1436 / 152 | 2115 / 297 | 34.8 / 35.8 |
| synaprpc | get | 32,538 (52.1%) | 35,292 (9.2%) | -7.8% | 114 / 106 | 231 / 209 | 35.4 / 34.1 |
| synaprpc | set | 21,358 (20.1%) | 32,975 (9.6%) | -35.2% | 170 / 112 | 319 / 220 | 33.4 / 33.8 |
| synaprpc | list | 22,732 (19.4%) | 33,333 (6.5%) | -31.8% | 166 / 111 | 302 / 219 | 15.6 / 15.2 |
| synaprpc | set-pipelined | 166,644 (15.9%) | 330,804 (11.3%) | -49.6% | 291 / 166 | 592 / 318 | 36.0 / 35.8 |

### Durability `always` (WAL, fsync before every acknowledgement)

| protocol | workload | 1.4.0 ops/s (spread) | 1.3.2 ops/s (spread) | ops/s change | p50 µs 1.4.0 / 1.3.2 | p99 µs 1.4.0 / 1.3.2 | peak RSS MiB 1.4.0 / 1.3.2 |
|---|---|---:|---:|---:|---:|---:|---:|
| resp3 | get | 35,703 (10.9%) | 35,569 (27.3%) | +0.4% | 106 / 105 | 215 / 216 | 33.8 / 34.2 |
| resp3 | set | 1,212 (21.3%) | 35,394 (8.7%) | -96.6% | 2963 / 106 | 10735 / 218 | 17.3 / 33.7 |
| resp3 | list | 1,261 (42.5%) | 34,956 (12.7%) | -96.4% | 2876 / 107 | 7468 / 219 | 15.4 / 15.1 |
| resp3 | set-pipelined | 1,246 (11.8%) | 383,621 (11.3%) | -99.7% | 48415 / 153 | 99698 / 296 | 17.3 / 35.3 |
| synaprpc | get | 34,595 (10.5%) | 34,190 (8.4%) | +1.2% | 108 / 109 | 215 / 220 | 36.9 / 34.2 |
| synaprpc | set | 1,237 (4.4%) | 33,620 (5.6%) | -96.3% | 2878 / 110 | 7301 / 221 | 17.7 / 34.0 |
| synaprpc | list | 1,140 (8.3%) | 32,376 (20.8%) | -96.5% | 3032 / 112 | 9822 / 226 | 15.5 / 15.1 |
| synaprpc | set-pipelined | 15,275 (8.7%) | 334,527 (9.5%) | -95.4% | 3893 / 165 | 9639 / 314 | 31.7 / 36.2 |

## Reading the numbers

- On this machine, run-to-run noise is large: most cells spread 5% to 20%
  across their five runs. A difference of a few percent between the two
  versions means nothing. One `periodic` / `synaprpc` / `get` run of 1.4.0
  ran at 18,778 ops/s, which explains that cell's 52% spread. Its median is
  unaffected.
- A client with one request in flight is limited by its round trip, about
  100 µs over loopback on this machine. Four such clients reach about 35,000
  ops/s in every version. More clients or pipelining raise the total: with
  persistence off, 64 clients reach about 80,000 ops/s, and 4 clients
  pipelining 16 requests reach about 380,000 RESP3 SETs per second.
- 1.3.2's numbers do not change with the durability mode for any write
  workload, because 1.3.2 did not log RESP3 or SynapRPC writes. In 1.3.2
  only MULTI/EXEC transactions on those transports reached the WAL
  (`protocol/resp3/command/advanced.rs` and
  `protocol/synap_rpc/dispatch/advanced.rs`, `log_transaction`). 1.4.0
  sends every write through `persistence/durable.rs`, so the WAL holds it
  before the reply goes out. See [guarantees](../operations/guarantees.md).
- The lower peak RSS of 1.4.0's `always` write cells, 17 MiB against 34 MiB,
  does not mean 1.4.0 uses less memory. At about 1,200 ops/s, those cells
  write fewer distinct keys in 5 s.

## Why writes with the WAL on are slower

### The path of one write in 1.4.0

A RESP3 or SynapRPC write takes these steps in `persistence/durable.rs`. It
locks the ordering stripes of its keys (`persistence/order.rs`; without
contention this is a `try_lock`). It applies the write to the store. It
queues the record on the WAL writer's channel while it still holds the
stripes, then releases them. Last, it waits for the WAL's acknowledgement
(`Ordered::commit`, `persistence/layer.rs`).

The writer task (`persistence/wal_async.rs`) takes everything already
queued, up to 1,000 records, as one group. It encodes and buffers the
records, flushes the group to the OS, fsyncs if the mode requires it, and
then answers every record in the group. The reply to the client goes out
only after that answer.

### Evidence

**1. Pipeline depth.** These runs used the 1.4.0 build, 4 clients and
`set-pipelined` at several depths. Each cell is the median of 3 runs of 3 s
(`perf_baseline.py --no-build --runs 3 --duration 3 --durability off,never,always
--workloads set-pipelined --pipeline <depth>`):

| depth | mode | RESP3 ops/s | RESP3 p50 µs | SynapRPC ops/s | SynapRPC p50 µs |
|---:|---|---:|---:|---:|---:|
| 1 | off | 36,867 | 102 | 33,973 | 108 |
| 1 | never | 26,435 | 144 | 24,266 | 156 |
| 1 | always | 1,158 | 2,932 | 1,256 | 2,849 |
| 4 | off | 129,967 | 115 | 114,502 | 126 |
| 4 | never | 40,558 | 384 | 66,884 | 208 |
| 4 | always | 1,262 | 11,706 | 4,696 | 2,965 |
| 16 | off | 384,843 | 152 | 334,722 | 165 |
| 16 | never | 47,790 | 1,304 | 182,001 | 279 |
| 16 | always | 1,085 | 49,510 | 16,483 | 3,517 |
| 64 | off | 754,158 | 298 | 580,000 | 323 |
| 64 | never | 50,696 | 4,972 | 341,815 | 530 |
| 64 | always | 1,130 | 197,980 | 29,309 | 8,082 |

With the WAL on, RESP3 throughput stays flat as the depth grows. Its batch
latency grows in step with the depth: about 80 µs per write in `never` and
about 3 ms per write in `always`. So each write of a batch waits for its
own group commit. SynapRPC keeps scaling: at depth 64 it reaches 342,000
ops/s in `never`, 59% of its `off` rate.

**2. Client count.** These runs used the `set` workload, with one request in
flight per client and the same 3 runs of 3 s (`--workloads set --clients <n>`):

| clients | mode | RESP3 ops/s | SynapRPC ops/s |
|---:|---|---:|---:|
| 1 | off / never / always | 10,814 / 6,935 / 526 | 9,857 / 6,421 / 572 |
| 4 | off / never / always | 35,695 / 26,458 / 1,224 | 33,979 / 22,955 / 1,215 |
| 16 | off / never / always | 71,054 / 58,714 / 4,775 | 71,383 / 62,311 / 4,917 |
| 64 | off / never / always | 80,260 / 74,098 / 16,922 | 74,824 / 74,683 / 18,373 |

The WAL itself batches well. Writes from different connections share group
commits. At 64 clients, `never` is within 8% of `off`, and `always`
throughput grows with the client count because each fsync covers more
writes. So the RESP3 limit is per connection, not in the WAL.

**3. The WAL in isolation.**
[`examples/wal_ack_probe.rs`](../../crates/synap-server/examples/wal_ack_probe.rs)
drives the server's `AsyncWAL`, with a timing wrapper around its file sink,
on the same drive
(`cargo run --release --locked -p synap-server --example wal_ack_probe`):

| measurement (p50) | `never` | `always` |
|---|---:|---:|
| A. one append, acknowledgement awaited | 61.6 µs | 1,457 µs |
| A. ... of which the group's flush (the write handed to the OS) | 34.7 µs | 42.2 µs |
| A. ... of which the group's fsync | - | 1,381 µs |
| B1. 16 appends, acknowledgements awaited one by one | 960.6 µs | 24,423 µs |
| B2. 16 appends queued, then awaited together | 77.1 µs (15.6 records per group) | 1,476 µs (16 per group) |
| C. 4 writers, one append in flight each (throughput, not p50) | 51,400 appends/s, 2.0 records per group | 1,289 appends/s, 2.0 records per group |
| D. the bare `write` of one ~110-byte frame on the calling thread | 2.0 µs | |
| E. an empty `spawn_blocking` round trip | 15.9 µs | |

B1 is what a RESP3 connection does with a pipelined batch. B2 is what
SynapRPC gets. In `never` mode, B2 is 12.5 times faster than B1, and in
`always` mode 16.5 times faster. Rows A, D and E show the other cost. The
WAL file is a `tokio::fs::File` behind a `BufWriter`, and tokio performs
every file write on its blocking thread pool (`tokio::fs::File::poll_write`
runs the write through `spawn_mandatory_blocking`, and the flush waits for
that task). As a result, the group's flush takes 34.7 µs, while the write
system call it wraps takes 2 µs.

### (a) The expected cost: logging writes 1.3.2 never logged

- Each acknowledged write now waits for one group commit: the record is
  encoded, written to the OS, and, in `always` mode, fsynced. With one write
  in flight per client, this adds about 50 to 60 µs to a request's p50 in
  `never`/`periodic` (for example, 160 µs against 105 µs for RESP3 `set` in
  `never`). With four clients, that is the -30% to -37% in the `set` and
  `list` cells.
- In `always` mode, each group waits for an fsync of about 1.4 ms on this
  drive (probe row A). In the probe, four writers with one write each share
  groups of about two. A write that arrives during an fsync waits for that
  fsync and then for its own group's, which matches the server's p50 of
  about 2.9 ms per request and about 1,200 ops/s for four clients. This is
  the price of the guarantee that `always` documents. More writes per
  group raise throughput, through more clients or through pipelining (SynapRPC
  `set-pipelined`: 15,275 ops/s).
- SynapRPC `set-pipelined` in `never`/`periodic` drops from about 330,000 to
  about 170,000 ops/s, even though the batch shares group commits as it
  should. What remains is the work of the group commits themselves. It also
  includes a task for every write: Thunder answers a request inline only when
  its dispatch finishes on the first poll, and a write that waits for the WAL
  does not, so Thunder moves it to a task of its own (thunder-rpc
  `server/listener.rs`). This run does not separate those two costs.

### (b) Avoidable overhead

**1. RESP3 waits for each write of a pipelined batch in turn.** This is the
main avoidable cost. It is why RESP3 `set-pipelined` loses 89% in
`never`/`periodic` while SynapRPC loses 47% to 50%. It is also why RESP3
stays at about 1,250 ops/s in `always`, where SynapRPC reaches 15,275.

`protocol/resp3/server.rs::handle_connection` reads one command, awaits
`dispatch`, and only then parses the next command. `dispatch` returns only
after the write's acknowledgement arrives. A batch of 16 writes therefore
makes 16 group commits, one after another, each holding one record. With
fsync, that is 16 fsyncs. The connection already holds back the replies of
a pipelined batch until its read buffer is drained (the "pipeline-aware
flush"), so nothing would be lost if the batch's writes shared one group
commit.

Proposed fix, in two files:

- `persistence/layer.rs`: add a deferral scope, a `tokio::task_local!`
  that collects the `Submitted` handles queued while it is active.
  `Ordered::commit`, `commit_all` and `commit_transaction` still queue their
  record while they hold the stripes, so memory order still equals WAL order.
  When a scope is active, they add the handle to the scope and return,
  instead of awaiting it. With no scope active, nothing changes.
- `protocol/resp3/server.rs`: run each `dispatch` inside the scope. Keep
  each reply in memory, together with the handles its command queued, instead
  of writing it at once. At the point where the loop flushes today (read
  buffer drained), or when the held replies reach a cap such as 1,000 (the
  writer's group size), await the held handles in order. Replace the reply of
  any command whose record failed with the error the command returns today
  (`ERR` followed by `write_not_durable_message`). Then write the replies and
  flush. No reply leaves before its write is durable. The in-memory effects
  are applied in command order exactly as today, and a failed write stays
  applied, as documented. Work that does not run in the connection's task
  (for example a script that writes from another thread) has no scope and
  keeps awaiting in place.

Expected gain: one group commit per batch instead of one per write. Probe
B1 against B2 shows 12.5 times less batch latency in `never` and 16.5 times
less in `always`. RESP3 `set-pipelined` should then reach at least
SynapRPC's numbers: about 4 times faster in `never`/`periodic` (from about
42,000 to about 170,000 to 180,000 ops/s) and about 12 times faster in
`always` (from about 1,250 to about 15,000 ops/s). It should also keep
scaling with depth as SynapRPC does.

**2. Each group commit's write goes through tokio's blocking thread pool.**
This is a smaller cost, about half of every acknowledgement in
`never`/`periodic`.

Per group, the flush costs 34.7 µs, for a write system call of 2 µs and an
empty pool round trip of 15.9 µs. The `fsync` (`File::sync_all`) makes one
more round trip.

Proposed fix, in `persistence/wal_async.rs`: run the writer loop on a
dedicated OS thread that owns a `std::fs::File` behind a
`std::io::BufWriter`. It would receive requests through a blocking channel
with a timed receive, for the `periodic` timer; for example `std::sync::mpsc`
with `recv_timeout` in place of the tokio unbounded channel. It would answer
through the existing oneshot senders, which can be sent from any thread.
`WalSink` becomes a synchronous trait, which keeps the failure-injection
seam. `enqueue` and `mark` are already synchronous, so callers do not change.

Expected gain: up to about 33 µs less per group commit, so a write's
acknowledgement takes about 30 µs instead of 62 µs. For non-pipelined writes
in `never`/`periodic`, that is roughly a quarter more throughput (about
23,600 to about 29,000 ops/s with four clients; the `off` ceiling is about
35,000). It also shortens the group cycle that pipelined SynapRPC and the
fixed RESP3 path depend on. In `always` mode the gain is small, because the
fsync dominates.

Neither fix changes what is durable when the reply is sent. Both are left
for a later release; this page records the 1.4.0 behavior.
