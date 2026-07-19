# Proposal: phase3_kv-store-optimization

## Why
The KV store at `synap-server/src/core/kv_store.rs` has measurable bottlenecks that SIMD cannot touch (pointer-chasing in trie/HashMap traversal). An audit of the current code identified four concrete wins: a cryptographic hasher used for internal sharding, always-heap key allocation, shard-thrashing MGET, and probabilistic TTL expiration without an index. Each is independently measurable and the cumulative payoff is large on the hot path of every Redis-compatible workload.

## What Changes
Land four independent improvements, smallest-blast-radius first:

1. **Swap hasher** — replace `std::collections::hash_map::DefaultHasher` (SipHash) at `kv_store.rs:275` with `ahash::AHasher`. Apply to both shard selection and the inner per-shard HashMap. Documented trade-off: ahash is not DoS-resistant; KV is behind authenticated RPC so this is acceptable.
2. **Inline short keys** — introduce a `KeyBuf` alias backed by `compact_str::CompactString` for keys ≤24 bytes. Heap allocation only for long keys. The `radix_trie` storage path keeps `String` because the crate does not impl `TrieKey` for `CompactString`.
3. **Shard-aware MGET** — `kv_store.rs:802–811` currently calls `self.get(key)` in a loop, re-locking the shard for every key. Bucket inputs by shard index, acquire each shard's read lock exactly once, drain all keys for that shard, preserve input order on output.
4. **Per-shard TTL min-heap** — replace probabilistic sampling at `kv_store.rs:857–916` with a `BinaryHeap<Reverse<(expires_at, KeyBuf)>>` per shard. Active expiration pops earliest entries. Stale entries (key overwritten with a new TTL) are detected by re-checking the live `expires_at` before eviction. Sampling stays as a fallback path for shards in HashMap mode if heap maintenance regresses writes.

Each item is a separate phase in `tasks.md` so it can be benchmarked in isolation against `redis-comparison-bench`.

## Impact
- Affected specs: none new; document hasher trade-off in `docs/kv-store.md` (create if missing)
- Affected code: `synap-server/src/core/kv_store.rs`, `synap-server/src/core/types.rs`, `synap-server/Cargo.toml` (add `ahash`, `compact_str`)
- Breaking change: NO — wire format and external API unchanged
- User benefit: target ≥1.5× GET/SET, ≥2× MGET (64 keys across shards), bounded TTL eviction latency under high-churn workloads. No regression on existing `redis-comparison-bench` p99.

## Out of scope
- Replacing `radix_trie` with ART (larger refactor, separate task if benches justify)
- SIMD anywhere in this task
- Persistence / disk tier changes
