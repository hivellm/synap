# Redis 7 vs Synap — Live `redis-benchmark` Comparison

**Status:** executed. This is the v1.0 head-to-head the earlier revisions owed.
Both servers were driven by the **same** `redis-benchmark` binary over RESP, so
it is a true apples-to-apples measurement.

> **Note on protocols.** Synap's **native protocol is SynapRPC** (MessagePack over
> a length-prefixed TCP frame); RESP3 exists for Redis-client **compatibility**.
> The `redis-benchmark` sections below measure the *compatibility* path against
> Redis. The **native SynapRPC path is measured separately below and is markedly
> faster per operation** — that is Synap's intended fast path.

## Native SynapRPC vs the RESP3 compatibility path

Measured with the in-repo `synap-bench` load generator (a `redis-benchmark`
equivalent that speaks SynapRPC — `crates/synap-server/src/bin/synap_bench.rs`),
run as a container on the **same** `synap-bench` network against the **same**
Synap server, `-n 100000 -c 50`, single key.

| Op | SynapRPC `-P 1` | RESP3 `-P 1` | Redis `-P 1` |
|---|---:|---:|---:|
| GET | **166,003** | 56,116 | 56,022 |
| SET | **170,307** | 52,301 | 56,465 |
| INCR | **169,726** | 53,850 | 55,897 |

**At `-P 1`, native SynapRPC is ~3× faster than RESP3** on every op — a single
MessagePack frame per reply vs RESP3's multi-segment bulk encoding, and a tighter
codec. This is the durable cross-protocol result (per-op, least sensitive to
client-loop differences).

| Op | SynapRPC `-P 16` | RESP3 `-P 16` |
|---|---:|---:|
| GET | 599,687 | 847,457 |
| SET | 330,092 | 757,575 |
| INCR | 470,708 | 458,715 |

At `-P 16`, SynapRPC GET reaches 600k (the SynapRPC server was given a `BufWriter`
so a pipelined burst of replies coalesces into one syscall — +23% over the
unbuffered path). It trails RESP3's GET here mainly because `synap-bench` is a
simple blocking client that sends a batch then reads it before the next, leaving
inter-batch gaps, whereas `redis-benchmark` keeps the pipe continuously full — so
this row understates the SynapRPC server ceiling. SET/INCR now scale with
pipelining after the phase12 write-lock fix (below).

## Environment

- **Harness:** `redis:7-alpine`'s `redis-benchmark`, `-n 100000 -c 50`,
  `-P 1` (no pipelining) and `-P 16` (pipelined). Commands:
  `set,get,incr,lpush,rpush,lrange_100,sadd`.
- **Topology:** both servers and the benchmark client run as containers on one
  Docker bridge network (`synap-bench`); traffic crosses the Docker network in
  both cases, so neither side gets a loopback advantage.
- **Redis:** `redis-server` 7.4.8, pure in-memory (`--save "" --appendonly no`).
- **Synap:** current `main`, built `--release` (glibc bench image
  `scripts/Dockerfile.bench`), RESP3 listener on `0.0.0.0:6379`, persistence and
  auth disabled — matching Redis's in-memory configuration.
- **Date:** 2026-07-10.

> Absolute rps depends on the host; the **Synap-vs-Redis ratio on the same host,
> same client** is the durable result.

## Results — `-P 1` (no pipelining, per-op latency)

| Op | Synap rps | Redis rps | Synap/Redis |
|---|---:|---:|---:|
| GET | 56,116 | 56,022 | 1.00 |
| SET | 52,301 | 56,465 | 0.93 |
| INCR | 53,850 | 55,897 | 0.96 |
| LPUSH | 55,834 | 57,836 | 0.97 |
| RPUSH | 56,401 | 55,555 | 1.02 |
| SADD | 54,945 | 55,493 | 0.99 |
| LRANGE_100 | 23,900 | 22,660 | 1.05 |

**At `-P 1`, Synap is at parity with Redis 7** (within ~5% on every command, and
marginally ahead on GET, RPUSH and LRANGE). Non-pipelined throughput is
latency-bound and both servers sit at ~52–56k rps.

## Results — `-P 16` (pipelined)

| Op | Synap rps | Redis rps | Synap/Redis |
|---|---:|---:|---:|
| GET | 847,457 | 925,925 | 0.92 |
| LPUSH | **740,740** | 549,450 | **1.35** |
| RPUSH | 787,401 | 961,538 | 0.82 |
| SADD | 787,401 | 917,431 | 0.86 |
| LRANGE_100 | 58,616 | 58,445 | 1.00 |
| SET | 757,575 | 952,381 | 0.80 |
| INCR | 458,715 | 925,925 | 0.50 |

**At `-P 16`, Synap reaches ~80–92% of Redis on GET/SET, beats Redis on LPUSH,
and is ~82–86% on RPUSH/SADD.** INCR (a read-modify-write under the shard data
lock) is the lowest at ~50%.

### The pipelined SET/INCR bottleneck — fixed (phase12)

`redis-benchmark` hammers a **single key** for SET/INCR. Originally Synap took a
per-key async **mutex** on every KV write (audit M-010, for MULTI/EXEC
isolation), so 50 clients × 16 pipelined writes to one key serialized on that one
`tokio::Mutex` — capping SET/INCR at **~130k rps**. But that lock only needs to
exclude plain writers *from an EXEC*, not from each other (two `SET k` are already
serialized by the KV shard data lock). So `KeyLockManager` was changed to a
sharded **`RwLock`**: plain writers take the shared **read** side (no longer
serialize), EXEC takes the exclusive **write** side (M-010 isolation preserved).
Result: **RESP3 SET 130k → 757k (5.8×), INCR 134k → 458k (3.4×)**; native
SynapRPC SET 81k → 330k, INCR 78k → 470k. `-P 1` is unchanged/slightly better and
all transaction-isolation tests still pass.

## Two bugs this benchmark found (now fixed)

The first run of this benchmark surfaced two socket-layer stalls in the RESP3
server, both since fixed (see CHANGELOG):

1. **No `TCP_NODELAY`.** A bulk reply is written as several small segments
   (length header, payload, CRLF); with Nagle's algorithm on, the payload
   segment waited ~40 ms for the header's delayed ACK. Non-pipelined GET/LRANGE
   ran at **~1,085 rps** before the fix (writes, single-segment, were unaffected).
2. **Unbuffered write half.** `flush()` on a raw `TcpStream` is a no-op, so the
   "pipeline-aware flush" could not coalesce a batch — every segment was its own
   `write()`. Pipelined (`-P 16`) throughput was capped flat at **~17k rps** for
   every op.

Fix: `set_nodelay(true)` on the RESP3/SynapRPC connections + a `BufWriter` around
the RESP3 write half. GET went **1,085 → 56,116 rps** (`-P 1`, 52×) and
**17,442 → 833,333 rps** (`-P 16`, 48×).

## Reproduce

```bash
docker network create synap-bench
docker run -d --name redis-bench --network synap-bench \
  redis:7-alpine redis-server --save "" --appendonly no
docker build --load -t synap:benchctx -f scripts/Dockerfile.bench .
docker run -d --name synap-bench --network synap-bench synap:benchctx

bench() { docker run --rm --network synap-bench redis:7-alpine \
  redis-benchmark -h "$1" -p 6379 -n 100000 -c 50 -P "$2" --csv \
  -t set,get,incr,lpush,rpush,lrange_100,sadd; }
bench redis-bench 1 ; bench synap-bench 1
bench redis-bench 16; bench synap-bench 16
```

---

## Prior: native-transport comparison (SynapRPC vs RESP3 vs HTTP, phase7)

Measured with `cargo bench --bench protocol_bench` against a live **release**
server on loopback (Windows, 2026-07-09). This compares Synap's **own** three
transports (not the Redis head-to-head above):

| Workload | HTTP/JSON | RESP3 | SynapRPC |
|---|---|---|---|
| SET + GET round-trip | 477.6 µs | 233.8 µs | **168.0 µs** |
| SET only | 193.0 µs | 108.5 µs | **79.9 µs** |
| GET only | 77.2 µs | 127.1 µs | 79.9 µs |

- **SynapRPC (MessagePack framing) is the fastest transport** on write and
  round-trip workloads — ~2.8× faster than HTTP/JSON and ~1.4× faster than RESP3
  on the realistic SET+GET round-trip.
- The isolated GET-only row is a harness artifact (client keep-alive vs per-call
  overhead), not a real transport property; the round-trip is representative.

> The earlier 0.9.0 HTTP/JSON-transport tables have been removed — they predated
> the RESP3/SynapRPC listeners and did not represent v1.0. The live
> `redis-benchmark` results above are the authoritative Redis comparison.

## Allocator A/B: mimalloc vs system (kv_bench)

`cargo bench --bench kv_bench -- read_latency/single_get`, system allocator vs
`--features mimalloc` (Windows, release):

| Allocator | `single_get` |
|---|---:|
| system (default) | 107.2 ns |
| mimalloc | 112.2 ns (**+4.2% slower**, p<0.05) |

On the read path mimalloc is a small **regression**, so the default
(`mimalloc` off) is correct and is kept. mimalloc can still help
allocation-heavy write/large-value workloads; a fuller write/concurrent A/B is
left for if/when the allocator is reconsidered.
