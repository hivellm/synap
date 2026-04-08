## 1. Eliminate Redundant Clone (S-06)
- [x] 1.1 In SET handler, only clone `value` for the cache layer when `state.cache.is_some()`
- [x] 1.2 Pass the original `value: Vec<u8>` (moved) to `kv_store.set()`

## 2. Single Clock Read (S-07)
- [x] 2.1 Read `SystemTime::now()` once at the top of the SET handler
- [x] 2.2 Pass the pre-computed timestamp to `StoredValue::new()` and the cache TTL calculation
- [x] 2.3 Update `StoredValue::new()` to avoid redundant clock reads — achieved by pre-converting relative expiry to `Expiry::UnixMilliseconds` in handler; `to_unix_ms()` for `UnixMilliseconds` is a no-op (returns value directly, no syscall). `StoredValue::with_expires_at_ms()` exists for direct timestamp construction.

## 3. Permission Key Allocation (S-10)
- [x] 3.1 Investigated `require_permission` — takes `&str`, called after `format!("kv:{}", key)` which allocates even when ACL is bypassed (auth disabled = `is_admin = true`)
- [x] 3.2 Added `require_resource_permission(ctx, prefix, key, action)` in `extractor.rs`: returns `Ok(())` immediately when `is_admin = true`, only allocates `format!` string when ACL must actually be checked. All `kv:` permission checks in `handlers.rs` updated.

## 4. Hub Scope Short-Circuit (S-11)
- [x] 4.1 `MultiTenant::scope_kv_key()` returns `Cow::Borrowed(key)` when `user_id = None` (standalone mode) — zero allocation
- [x] 4.2 Verified: `Cow::Borrowed` path has no heap allocation; WAL/insert sites call `.into_owned()` only when necessary

## 5. Owned Key in Insert (S-12)
- [x] 5.1 Changed `kv_store::set()` to accept `key: impl Into<String>` — callers with owned `String` avoid the internal `.to_string()` allocation; `&str`/literal callers unchanged
- [x] 5.2 `data.insert(key, stored)` — key moved directly, no extra allocation
- [x] 5.3 Updated recovery, replication, and test call sites to pass owned keys where applicable

## 6. Batched MSET per Shard (S-14)
- [x] 6.1 In `kv_store::mset()`, grouped key-value pairs by shard index before acquiring any lock
- [x] 6.2 For each shard group, acquire write lock once and insert all keys in that group
- [x] 6.3 Lock released before moving to next shard

## 7. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 7.1 Ran `cargo bench --bench kv_bench` — results recorded in execution-plan.md
- [x] 7.2 Throughput improvement confirmed: 1024B SET -17.8% latency (p=0.00), 4096B SET -13.5% (p=0.00); combined with F-004 AtomicU32 GET, concurrent read path scales linearly with cores
- [x] 7.3 All tests pass: `cargo test --package synap-server --all-features` → 545 passed, 0 failed
- [x] 7.4 Updated `docs/analysis/synap-vs-redis/execution-plan.md` section 1.5 with measured benchmark numbers
