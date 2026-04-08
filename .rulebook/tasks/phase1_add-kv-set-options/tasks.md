## 1. Expiry Enum and Millisecond Storage (S-02)
- [x] 1.1 Add `Expiry` enum to `core/types.rs`: `Seconds(u64)` | `Milliseconds(u64)` | `UnixSeconds(u64)` | `UnixMilliseconds(u64)`
- [x] 1.2 Change `StoredValue::Expiring.expires_at` from `u32` to `u64` (milliseconds since epoch)
- [x] 1.3 Update all `expires_at` calculation sites to use the new `u64` millisecond representation
- [x] 1.4 Update `cleanup_expired()` comparison to use `u64` millisecond timestamps

## 2. SetOptions Struct (S-01)
- [x] 2.1 Add `SetOptions` struct to `core/types.rs` with fields: `if_absent: bool`, `if_present: bool`, `keep_ttl: bool`, `return_old: bool`
- [x] 2.2 Add `old_value: Option<Vec<u8>>` to the SET return type for `return_old` support (`SetResult`)

## 3. kv_store SET Implementation
- [x] 3.1 Add `kv_store::set_with_opts()` accepting `SetOptions` and `Option<Expiry>`
- [x] 3.2 Implement NX check: if `if_absent=true` and key exists, return without inserting (under shard write lock)
- [x] 3.3 Implement XX check: if `if_present=true` and key does not exist, return without inserting
- [x] 3.4 Implement KEEPTTL: if `keep_ttl=true`, preserve existing `expires_at_ms` from old entry
- [x] 3.5 Implement GET: if `return_old=true`, clone old value before replacing and return it

## 4. HTTP Handler Update
- [x] 4.1 Add `nx`, `xx`, `keepttl`, `get` boolean fields to `SetRequest` in `handlers.rs`
- [x] 4.2 Add `expiry: Option<Expiry>` field to `SetRequest` (supersedes `ttl` for new callers)
- [x] 4.3 Update HTTP SET handler to pass `SetOptions` and resolved `Expiry` to `kv_store.set_with_opts()`
- [x] 4.4 Return `written: bool` and `old_value` in `SetResponse` when `get: true` was requested

## 5. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 5.1 Export `Expiry`, `SetOptions`, `SetResult` from `core/mod.rs`
- [x] 5.2 Write tests: NX (absent/present), XX (absent/present), GET (returns old value), KEEPTTL
- [x] 5.3 Write contention test: 100 concurrent `SET lock NX EX 30` — exactly 1 MUST succeed
- [x] 5.4 Write test: SET with PX expiry, assert millisecond-precision TTL stored correctly (`remaining_ttl_ms`)
- [x] 5.5 Run `cargo test --package synap-server` — 540 tests pass (was 535)
