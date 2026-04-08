## 1. AtomicU32 Migration (core/types.rs)
- [x] 1.1 Change `last_access: u32` to `last_access: AtomicU32` in `StoredValue::Expiring`
- [x] 1.2 Update `StoredValue::new()` and any constructors that initialize `last_access`
- [x] 1.3 Update `Clone` implementation for `StoredValue` — `AtomicU32` does not impl `Clone` automatically; use `AtomicU32::new(self.last_access.load(Relaxed))`

## 2. GET Read Lock (kv_store.rs)
- [x] 2.1 Change `shard.data.write()` in the GET path to `shard.data.read()`
- [x] 2.2 Update `last_access` update site: use `entry.last_access.store(now_secs, Relaxed)` under the read lock
- [x] 2.3 Verify no other write operations are performed under this lock that would require write access

## 3. Eviction Compatibility
- [x] 3.1 Update `evict_candidates()` to read `last_access` via `.load(Relaxed)` for comparison
- [x] 3.2 Confirm eviction logic still correctly identifies least-recently-accessed entries

## 4. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 4.1 Write concurrent read benchmark: spawn 16 reader threads on one shard, measure throughput before and after — assert ≥8x improvement
- [x] 4.2 Write correctness test: GET updates last_access such that LRU eviction prefers older entries
- [x] 4.3 Run `cargo check` then `cargo test --workspace --all-features` and confirm all pass
