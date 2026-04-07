## 1. Atomic Stats (S-04)
- [x] 1.1 Replace `Arc<RwLock<KVStats>>` with individual `AtomicU64`/`AtomicI64` fields in `core/types.rs`
- [x] 1.2 Update all `stats.write()` sites in `kv_store.rs` to use `fetch_add(_, Relaxed)`
- [x] 1.3 Update all `stats.read()` sites to use `load(Relaxed)`

## 2. Memory Accounting (S-05)
- [x] 2.1 In `kv_store.rs::set()`, capture `old_size` from `data.insert()` return value and subtract from `total_memory_bytes`
- [x] 2.2 In `kv_store.rs::delete()`, subtract the removed entry's size from `total_memory_bytes`
- [x] 2.3 In `kv_store.rs::cleanup_expired()`, subtract each evicted entry's size

## 3. Max Value Size Guard (S-13)
- [x] 3.1 Add `max_value_size_bytes: Option<usize>` to `KVConfig` in `core/types.rs`
- [x] 3.2 In `handlers.rs` SET handler, reject values exceeding `max_value_size_bytes` before any allocation

## 4. WAL Write-Ahead Semantics (S-08)
- [x] 4.1 Add `durability: DurabilityMode` enum (`Sync` | `Async`) to WAL config in `persistence/types.rs`
- [x] 4.2 Move WAL log call in `handlers.rs` SET handler to BEFORE `kv_store.set()` when mode is `Sync`
- [x] 4.3 When WAL write fails in `Sync` mode, return error without writing to memory
- [x] 4.4 Document `Async` mode semantics honestly (response does not guarantee durability)

## 5. INCR/DECR TTL Preservation (S-16)
- [x] 5.1 In INCR/DECR handlers, read existing entry's `expires_at` before modifying
- [x] 5.2 Write new value as `Expiring` with original `expires_at` if entry had a TTL
- [x] 5.3 Use `checked_add`/`checked_sub` and return clean error on overflow

## 6. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 6.1 Update `docs/analysis/synap-vs-redis/` with benchmark numbers
- [x] 6.2 Write stress test: 1M SET-overwrite cycles, assert `total_memory_bytes` matches single entry <!-- test_memory_accounting_overwrite_stress -->
- [x] 6.3 Write test: concurrent SET on 16 threads, confirm no contention/deadlock <!-- test_concurrent_set_no_lock_contention -->
- [x] 6.4 Write test: INCR on key with TTL, assert TTL preserved after increment <!-- test_incr_preserves_ttl -->
- [x] 6.5 Write test: `max_value_size_bytes` config defaults to None and is settable <!-- test_max_value_size_config_field -->
- [x] 6.6 Run `cargo check` then `cargo test --package synap-server` — 535 tests pass
