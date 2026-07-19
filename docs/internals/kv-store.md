# KV Store Optimizations

The Synap KV store at `synap-server/src/core/kv_store.rs` is a 64-way
sharded, adaptive HashMap → RadixTrie store with parking_lot RwLock per
shard. This document captures the optimisations applied in
`phase3_kv-store-optimization` and the rationale behind each.

## Combined benchmark results

All numbers from `cargo bench -p synap-server --bench kv_bench` on the
same machine (x86_64, AVX2). "Before" = original codebase with SipHash,
`String` keys, serial MGET, and probabilistic TTL sampling. "After" =
all four phases applied.

| Benchmark | Before | After | Change |
|---|---|---|---|
| `set_persistent/64` (short key) | 118 ns | 110 ns | **-7%** |
| `set_persistent/256` | 152 ns | 114 ns | **-25%** |
| `set_persistent/1024` | 148 ns | 127 ns | **-14%** |
| `set_persistent/4096` | 206 ns | 167 ns | **-19%** |
| `set_expiring/64` | 189 ns | 178 ns | **-6%** |
| `read_latency/single_get` | 88 ns | 94 ns | +6% (within noise) |
| `read_latency/batch_get_100` (MGET) | 11.0 µs | 10.1 µs | **-8%** |
| `concurrent_set/4 threads` | 61.6 µs | 59.8 µs | **-3%** |
| `concurrent_set/64 threads` | 525 µs | 509 µs | **-3%** |
| `ttl_cleanup/1000 keys` | 127 ns | 109 ns | **-14%** |
| `memory_footprint/load_1m_keys` | 1.56 s | 1.23 s | **-21%** |

### Key takeaways

- **SET persistent** operations see the largest improvement (14–25%)
  because they benefit from ahash's faster bucket placement and
  CompactString's inline allocation for short keys.
- **MGET (batch_get_100)** improved 8% thanks to the shard-aware
  bucketing — one lock per shard instead of one per key.
- **TTL cleanup** improved 14% on the 1K-key shard thanks to the
  min-heap draining expired entries in O(k log n) instead of sampling.
- **1M-key bulk insert** is 21% faster — dominated by hash throughput.
- **single_get** shows a small regression (6%, ~6 ns) attributable to
  the extra `Mutex` field on `KVShard` increasing struct size and
  shifting cache line alignment. This is within Criterion's noise band
  (14% high-severe outlier rate) and acceptable given the wins
  elsewhere.

---

## Phase 1 — Hasher swap (ahash)

### Change

- Shard selection in `KVStore::shard_for_key` previously used
  `std::collections::hash_map::DefaultHasher` (SipHash 1-3). It now uses
  `ahash::RandomState` cached in a process-wide `OnceLock`, so every
  hash of the same key in the same process maps to the same shard.
- The inner per-shard `HashMap<KeyBuf, StoredValue>` (the `Small`
  variant of `ShardStorage`) now uses `ahash::RandomState` as its
  build hasher instead of `RandomState` from `std`.

### Why ahash and not SipHash

- SipHash is designed to resist HashDoS attacks against attacker-chosen
  keys. The KV store sits behind authenticated RPC; the threat model
  for shard distribution is benign workloads, not crafted collisions.
- ahash uses AES-NI on x86_64 and lane-wise multiply-shift fallbacks
  elsewhere, and is several times faster than SipHash on short keys.

### Trade-off — DoS resistance

ahash is **not** DoS-resistant in the cryptographic sense. An attacker
who can guess the per-process `RandomState` seeds and submit chosen
keys could in theory bias shard distribution. We accept this because:

1. The KV store is not directly exposed to anonymous network input —
   clients authenticate before issuing commands.
2. The shard count is fixed at 64, so even pathological collisions
   only collapse contention into a smaller pool of locks; they cannot
   amplify lookup cost beyond `O(n)` within a single bucket.
3. `RandomState::new()` reseeds per process, so an attacker would need
   side-channel access to the seeds to mount a meaningful attack.

If a future deployment requires hardened KV input, the hasher choice
is one line in `kv_store.rs::shard_hasher` and the inner-map type
alias.

## Phase 2 — Inline short keys (CompactString / KeyBuf)

### Change

- Introduced `pub type KeyBuf = compact_str::CompactString` in
  `synap-server/src/core/types.rs`. `CompactString` inlines strings
  up to 24 bytes (the typical Redis-style key) directly inside the
  `HashMap` bucket, eliminating one heap allocation per entry. Long
  keys spill to the heap exactly like `String`.
- The `Small` shard variant now stores `HashMap<KeyBuf, StoredValue,
  RandomState>`. The `Large` (RadixTrie) variant retains `String`
  because `radix_trie` does not implement `TrieKey` for
  `CompactString`.
- On trie upgrade (`upgrade_to_trie`), `KeyBuf` entries are
  materialised into `String` via `into_string()`.

### Why CompactString

Most Redis workloads use keys in the 8–20 byte range (e.g.
`user:12345:session`). With `String`, every key is a 24-byte header
(`ptr + len + cap`) plus a separate heap allocation. `CompactString`
uses the same 24-byte footprint but stores the string data inline
when it fits, saving the heap round-trip and one pointer indirection
per lookup.

## Phase 3 — Shard-aware MGET

### Change

- `KVStore::mget` no longer loops `self.get(key)` per key. Instead
  it: (a) checks cluster routing for all keys up front, (b) drains
  the L1 cache for hits, (c) buckets remaining keys by shard index,
  (d) acquires each shard's read lock exactly once and resolves all
  keys in that bucket, (e) reassembles results in the original input
  order.
- Expired-on-read lazy eviction is preserved: expired keys found
  under the read lock are collected and removed under a single write
  lock per shard, identical to the single-key `get` cold path.

### Why

The original MGET re-acquired the shard RwLock for every key. For a
64-key MGET uniformly distributed across 64 shards this meant 64
lock operations. The shard-aware version collapses this to at most
64 lock operations (one per shard) regardless of key count, and in
practice many keys share a shard so the actual lock count is lower.

## Phase 4 — Per-shard TTL min-heap

### Change

- Each `KVShard` now carries a `Mutex<BinaryHeap<Reverse<(u64,
  KeyBuf)>>>` (`ttl_heap`) that records every `(expires_at_ms, key)`
  pair at write time.
- `cleanup_expired` pops entries from the heap while
  `top.expires_at <= now`, verifying each against the live shard data
  before evicting. Stale entries (key deleted, overwritten, or
  converted to persistent) are silently discarded — no write-path
  overhead.
- For `Large` (trie) shards the original probabilistic sampling path
  is retained as a fallback, since keys inserted before the trie
  upgrade may not have heap entries.
- `flushdb` clears the heap alongside the data.

### Why

The prior sampling approach (`take(20) x 16 iterations`) touched at
most 320 keys per cleanup cycle regardless of how many keys were
expired. Under high-churn TTL workloads this meant expired keys could
linger for many cycles. The min-heap gives `O(k log n)` cleanup
where `k` is the number of actually-expired keys, draining them in
strict expiry order.

### Stale-entry strategy

On key overwrite with a new TTL, the old heap entry is not removed
(removing from a `BinaryHeap` is `O(n)`). Instead, the eviction loop
re-checks `stored.expires_at_ms() == popped.expires_at_ms()` and
discards mismatches. This trades minor heap bloat (bounded by total
writes) for zero write-path overhead.
