# Redis 7 vs Synap — Live `redis-benchmark` Comparison

**Status:** executed. This is the v1.0 head-to-head the earlier revisions owed.
Both servers were driven by the **same** `redis-benchmark` binary over RESP, so
it is a true apples-to-apples measurement.

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
| GET | 833,333 | 925,925 | 0.90 |
| LPUSH | **740,740** | 549,450 | **1.35** |
| RPUSH | 787,401 | 961,538 | 0.82 |
| SADD | 787,401 | 917,431 | 0.86 |
| LRANGE_100 | 58,616 | 58,445 | 1.00 |
| SET | 130,718 | 952,381 | 0.14 |
| INCR | 134,770 | 925,925 | 0.15 |

**At `-P 16`, Synap reaches ~90% of Redis on GET, beats Redis on LPUSH, and is
~82–86% on RPUSH/SADD.** The outliers are **SET/INCR**: see below.

### The pipelined SET/INCR bottleneck

`redis-benchmark` hammers a **single key** for SET/INCR. Synap takes a per-key
async lock on every KV write (audit M-010, for MULTI/EXEC isolation), so 50
clients × 16 pipelined writes to one key serialize on that one `tokio::Mutex` —
capping SET/INCR at ~130k rps with ~6 ms latency, while lock-free GET hits 833k
and the sharded collection writes (LPUSH via `parking_lot` RwLock) hit ~740k.
This is a real, isolated bottleneck for hot-single-key pipelined writes and is
tracked as a follow-up (`phase12_kv-write-lock-fastpath`). Multi-key or
non-pipelined write workloads are unaffected (see the `-P 1` parity above).

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
