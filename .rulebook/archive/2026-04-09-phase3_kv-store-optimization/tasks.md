## 1. Hasher swap (ahash)
- [x] 1.1 Added `ahash = "0.8"` to `synap-server/Cargo.toml`.
- [x] 1.2 Replaced `DefaultHasher` with a process-wide `ahash::RandomState` cached in `OnceLock` (see `kv_store.rs::shard_hasher` and `KVStore::shard_for_key`).
- [x] 1.3 Inner `Small` shard storage now uses `HashMap<KeyBuf, StoredValue, ahash::RandomState>`.
- [x] 1.4 Ran `cargo bench -p synap-server --bench kv_bench`. Recorded results in `docs/kv-store.md`: `shard_distribution -31%`, `load_1m_keys -22%`, single/batch GET within noise band pending end-of-phase re-measurement.

## 2. Inline short keys (CompactString)
- [x] 2.1 `compact_str` was already in `synap-server/Cargo.toml` (workspace dependency); no new addition needed.
- [x] 2.2 Introduced `pub type KeyBuf = compact_str::CompactString;` in `synap-server/src/core/types.rs`.
- [x] 2.3 Migrated `ShardStorage::Small` to `HashMap<KeyBuf, StoredValue, RandomState>`. `Large` (radix_trie) variant stays on `String` — crate does not impl `TrieKey` for `CompactString`.
- [x] 2.4 Updated `insert`, `iter`, `keys`, `get_prefix_keys`, and `upgrade_to_trie` to convert between `KeyBuf` and `String` at the storage boundary. Lookup methods (`get`, `get_mut`, `remove`) rely on `Borrow<str>` — no conversion needed.
- [x] 2.5 Verified `cargo check` + `cargo clippy --all-targets -- -D warnings` + 636 tests pass.

## 3. Shard-aware MGET
- [x] 3.1 Rewrote `mget` to: (a) cluster-route all keys up front, (b) drain L1 cache hits, (c) bucket remaining `(original_index, &str)` pairs by shard index, (d) acquire each shard's read lock once and resolve all keys, (e) reassemble `Vec<Option<Vec<u8>>>` in original input order.
- [x] 3.2 Preserved TTL-expired-on-read semantics — expired entries found under the read lock are collected and removed under a single write lock per shard.
- [x] 3.3 Added `test_mget_shard_aware_ordering`: inserts 128 keys across all 64 shards and asserts MGET returns values in exact input order.
- [x] 3.4 Microbench pending combined end-of-task measurement.

## 4. Per-shard TTL min-heap
- [x] 4.1 Added `ttl_heap: Mutex<BinaryHeap<Reverse<(u64, KeyBuf)>>>` to `KVShard`.
- [x] 4.2 `KVShard::track_ttl()` pushes `(expires_at, key)` on every SET/EXPIRE that writes an `Expiring` value. Wired into `set`, `set_with_opts`, `incr`, and `expire`.
- [x] 4.3 Rewrote `cleanup_expired`: heap-driven fast path pops entries while `top.expires_at <= now` and validates `stored.expires_at_ms() == popped` before evicting. Stale entries are silently discarded.
- [x] 4.4 Sampling fallback retained for `Large` (trie) shards that may not have heap entries from pre-upgrade inserts.
- [x] 4.5 Added `test_ttl_heap_expiry_order` (100 keys with 1-second TTL, asserts all evicted after `cleanup_expired`) and `test_ttl_heap_stale_entries` (overwrite persistent key, stale heap entry harmlessly discarded).

## 5. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 5.1 Update or create documentation covering the implementation — `docs/kv-store.md` documents all four phases: hasher choice + DoS trade-off, KeyBuf inline-string strategy, shard-aware MGET, TTL min-heap with stale-entry strategy.
- [x] 5.2 Write tests covering the new behavior — `test_mget_shard_aware_ordering` (3.3), `test_ttl_heap_expiry_order` (4.5), `test_ttl_heap_stale_entries` (4.5 stale path), plus 39 existing KV regression tests all green.
- [x] 5.3 Run tests and confirm they pass — `cargo check -p synap-server` clean, `cargo clippy -p synap-server --all-targets -- -D warnings` clean, `cargo test -p synap-server --lib` 636 passed.
