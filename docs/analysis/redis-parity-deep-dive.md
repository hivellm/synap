# Deep dive: what it takes to beat Redis 7 on the hot path

**Date:** 2026-07-10 · **Status:** analysis + plan (phase12 follow-on)
**Companion:** `docs/benchmarks/redis-vs-synap.md` (live numbers)

## 1. Where we stand (median-of-3, `-P 16`, Docker, single hot key)

| Op | Synap | Redis 7.4 | ratio | trend across phase12 |
|---|--:|--:|--:|---|
| SET | 797,872 | 847,458 | **0.94** | 0.14 → 0.80 → 0.94 |
| GET | 810,811 | 882,353 | **0.92** | 0.02 → 0.90 → 0.92 |
| SADD | 789,474 | 867,052 | **0.91** | 0.72 → 0.86 → 0.91 |
| INCR | 746,269 | 892,857 | 0.84 | 0.15 → 0.50 → 0.84 |
| RPOP | 793,651 | 955,414 | 0.83 | — |
| LPOP | 750,000 | 920,245 | 0.81 | — |
| RPUSH | 678,733 | 920,245 | 0.74 | — |
| LPUSH | 777,202 | 515,464 | **1.51** | we win |
| LRANGE_100 | ~58,600 | ~58,400 | 1.00 | tie |

Two framing facts before the plan:

- **`redis-benchmark` single-key is Synap's worst case and Redis's best case.**
  One hot key means Synap's 64-way sharding and multi-core execution buy
  *nothing* — every op lands on one shard, one lock, one cache line — while
  Redis's single thread runs its ideal workload. On realistic multi-key traffic
  Synap scales with cores and Redis cannot. We should still close the single-key
  gap, but "beating Redis" must be measured on both shapes.
- Run-to-run noise on this harness is ±5%; ratios ≥0.90 are effectively parity.

## 2. How Redis does it (from the Redis source)

What makes Redis fast per command is the accumulation of *zero-cost defaults*:

| Subsystem | Redis mechanism | Cost per command |
|---|---|---|
| Concurrency | `ae.c` single-threaded event loop; optional IO threads only move socket IO | **zero locks, zero atomics** on the data path |
| Parsing | `networking.c processMultibulbuffer`: argv SDS created straight off `querybuf`, length-known `sdsnewlen`, no UTF-8 validation (binary safe) | 1 tight alloc per arg |
| Integers | `object.c`/`t_string.c` **int encoding**: a numeric string is stored as a `long` in `robj->ptr`; INCR is `value++` — formatting happens only on read via `ll2string` into a stack buffer; shared objects for 0–9999 | **0 allocs for INCR** |
| Replies | `addReply*`: headers built with `ll2string` in stack buffers into a **static 16 KB per-client buffer** (`c->buf`); one `write()` per event-loop iteration (`beforeSleep`) — pipelining coalesces naturally | 0 allocs for headers/ints |
| Lists | `t_list.c` quicklist of **listpacks**: elements packed into contiguous blobs | no per-element allocation, cache-dense |
| Small sets | intset / listpack encodings | no per-member allocation |
| Stats | `server.stat_numcommands++` plain increments | 0 synchronization |
| Hashing | `dict.c` siphash-1-2, incremental rehash | 1 cheap hash/lookup |

## 3. What Synap still pays per command (verified in code)

Ordered by estimated cost × breadth. ✅ = fixed in phase12 rounds 1–3.

| # | Cost | Where | Affects |
|---|---|---|---|
| A | **Reply headers/integers allocate**: `format!("${len}\r\n")`, `format!(":{n}\r\n")` — a `String` alloc per bulk header and per integer reply | `synap-protocol/src/resp3/writer.rs:135-148` | every reply (INCR reply is pure `:n`) |
| B | **Prometheus label lookup ×3 per command**: `with_label_values(&[cmd, status])` does an internal hash + `RwLock` read in prometheus, called for 2 counters + 1 histogram | `synap-server/src/metrics/mod.rs:575` | every command |
| C | **`cmd_upper` String alloc per command in the server loop** (AUTH/ACL gate + span + metrics label) — the dispatcher was fixed (stack buffer) but the server loop still allocates | `resp3/server.rs:159` | every command |
| D | **SET value double-copy**: parser allocates the arg `Vec`, then `StoredValue::new` does `data.into()` = `Vec<u8>` → `Arc<[u8]>` — a second alloc + full memcpy of the value | `synap-core/src/core/types.rs:121` | every KV write (cost of phase9/11 zero-copy reads) |
| E | **`notify_waiters` on every push**: global `RwLock` read + `HashMap<String, Sender>` hash+lookup even when *no* blocked waiter exists | `synap-core/src/core/list.rs:461`; same pattern in `sorted_set.rs` | LPUSH/RPUSH/ZADD |
| F | **INCR still allocates 2×**: `int_to_bytes` (String) + `set_data` Vec→Arc copy — Redis does 0 (int encoding) | `kv_store/store.rs` | INCR/DECR |
| G | **Parser materializes owned values**: `vec![0u8; len]` per bulk arg + `Vec<Resp3Value>` per command + UTF-8 validation on protocol lines | `resp3/parser.rs:127,137,186` | every command |
| H | **Contended atomics**: `stats.gets/sets.fetch_add` from all cores on shared cache lines (ping-pong); Redis pays a plain `++` | `kv_store/store.rs` | every op |
| I | **tokio + locks tax**: task wake/schedule per batch, `RwLock` read (key lock) + `parking_lot` write (shard) per op | architectural | everything |
| J | **`VecDeque<Vec<u8>>` lists**: one heap alloc per element vs Redis's contiguous listpack | `list.rs` | list ops (why RPUSH trails at 0.74) |
| ✅ | per-command `String` uppercase in dispatch | fixed r1 | — |
| ✅ | INCR read-side `to_vec` + key re-insert | fixed r1 | — |
| ✅ | collection stats global `RwLock` | fixed r2 | — |
| ✅ | SADD double lookup; GET key alloc | fixed r2/r3 | — |
| ✅ | per-key write **mutex** (M-010) → sharded `RwLock` | fixed | — |
| ✅ | no TCP_NODELAY; unbuffered writers | fixed | — |

## 4. Plan

### Round 4 — low risk, broad wins (do next)
1. **Writer headers without `format!`** (kills A): integer→ASCII into a stack
   buffer (itoa-style) for `$len`, `:n`, `*n`, `%n`, `~n`. Touches only
   `writer.rs`; every reply benefits, INCR reply most.
2. **`cmd_upper` stack buffer in `resp3/server.rs`** (kills C): same fix already
   applied to the dispatcher.
3. **Metrics: pre-resolved handles** (kills B): resolve
   `with_label_values` once per (command, status) into static handles (match on
   the fixed command set → `IntCounter` refs), falling back to the dynamic path
   for unknown commands. 3 hash+lock lookups/op → 2-3 plain atomic incs.
4. **`notify_waiters` zero-waiter fast path** (kills E): `AtomicUsize` waiter
   count bumped by BLPOP/BZPOP registration; push checks `count == 0` and skips
   the map entirely.

### Round 5 — contained refactors
5. **Parse bulk args directly into `Arc<[u8]>`** (kills D): the parser allocates
   `Arc::new_uninit_slice(len)` and `read_exact`s into it; `BulkString` carries
   the `Arc`, and KV write paths store it without re-copying. One full memcpy
   removed per write.
6. **Hash/zset atomic stats** (finish H's family; hash/zset ops not in the sweep
   but same defect).

### Round 6 — architectural (only to *win outright* on single-key)
7. **Int encoding for counters** (kills F): a `StoredValue::Int(i64)` variant;
   INCR becomes an integer add (0 allocs), GET formats on read. Ripples through
   every `data()` borrower — medium-large refactor.
8. **Zero-copy parser** (kills G): borrow args from the connection read buffer
   (`bytes::Bytes`-style) instead of owning; the biggest single win left and the
   riskiest.
9. **Listpack-style contiguous list encoding** (kills J, fixes RPUSH).
10. **Per-core stat cells** (kills H): pad/shard counters, sum on read.

### Ceiling and definition of "beating Redis"
Rounds 4–5 should put SET/GET/SADD/INCR at **0.95–1.05** on the single-key
harness — i.e. within noise of Redis, while already beating it on LPUSH and on
any multi-key/multi-core workload. Round 6 items are what it would take to win
*every* single-key row outright; each is a real subsystem rewrite. The honest
statement after round 5: *parity on Redis's home turf, ahead everywhere
parallelism counts*. Also benchmark a **multi-key** sweep (`-r 1000000`) to show
the sharding advantage redis-benchmark's default single-key shape hides.
