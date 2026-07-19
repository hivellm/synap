# Proposal: Add KV SET Options — NX, XX, GET, KEEPTTL, PX/EXAT/PXAT

## Why

Redis-compatible SET options are required to support the most common distributed patterns that
users expect from a KV cache. Without them, Synap cannot implement distributed locks, atomic
conditional updates, or millisecond-precision TTL — all of which are standard Redis use cases.

The current `SetRequest` struct accepts only `key`, `value`, and `ttl: Option<u64>` (seconds).
This blocks entire categories of real-world usage:

- Distributed locking requires `SET lock owner NX EX 30` (set only if absent)
- Cache stampede prevention requires `SET key value XX` (set only if present)
- Atomic test-and-set patterns require `SET key value GET` (return old value)
- Session management requires millisecond TTL (`PX`) and absolute expiry (`EXAT`, `PXAT`)
- Cache refresh without TTL reset requires `KEEPTTL`

These were identified in `set-deep-dive.md` as S-01 and S-02, the highest-priority feature gaps
in the SET path.

Source: docs/analysis/synap-vs-redis/ (set-deep-dive S-01, S-02; execution-plan Phase 1.2)

## What Changes

- ADDED: `SetOptions` struct with fields `if_absent: bool` (NX), `if_present: bool` (XX), `keep_ttl: bool`, `return_old: bool` (GET)
- ADDED: `Expiry` enum: `Seconds(u64)` | `Milliseconds(u64)` | `UnixSeconds(u64)` | `UnixMilliseconds(u64)`
- MODIFIED: `StoredValue::Expiring.expires_at` — change storage from `u32` seconds to `u64` milliseconds
- MODIFIED: `kv_store.rs::set()` — accept `SetOptions`, implement NX/XX check under the same shard write lock (no TOCTOU)
- MODIFIED: HTTP SET handler in `handlers.rs` — accept optional `options` field in `SetRequest`
- MODIFIED: All TTL calculation sites — update to use millisecond precision

## Impact

- Affected specs: specs/kv/spec.md
- Affected code: synap-server/src/core/types.rs, synap-server/src/core/kv_store.rs, synap-server/src/server/handlers.rs
- Breaking change: NO (existing `ttl` field remains; new `options` field is optional)
- User benefit: Distributed lock pattern works; conditional SET works; millisecond TTL enabled; atomic GET-and-SET available
