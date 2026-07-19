# Proposal: phase21_kv-watch-core-notifier

Source: docs/analysis/kv-watch-observable/ (F-002, F-004, F-005, F-006, F-007, F-008)

## Why

Synap has no way for a client to observe a key and receive its new value when it changes.
Keyspace notifications (`__keyspace@0__`) fire on every KV mutation but carry only the event
name, are opt-in via `notify_keyspace_events` (default OFF), and follow Redis-compat semantics
that cannot be extended with values. The post-mutation value is already in scope at every
notify site in `kv_store/store.rs` but is dropped. This phase builds the core primitive that
all watch surfaces (RPC, WebSocket, SDKs) will ride.

## What Changes

- New `KeyWatchNotifier` in `crates/synap-core` (sibling of `KeyspaceNotifier`), publishing
  to `__watch@0__:<key>` channels through the existing `PubSubRouter` — **always-on**,
  independent of `notify_keyspace_events`.
- MessagePack event envelope: `{ key, event, version, value?, truncated? }`. `version` is a
  per-key monotonic counter. `value` is the post-mutation value, omitted (with
  `truncated: true`) when it exceeds a configurable inline cap (default 64 KiB).
- Fast idle path: when the router has no subscriber matching `__watch@0__:<key>`, the notify
  site is a near-no-op (no serialization, no publish).
- All KV mutation sites thread the post-mutation value: set, del, setex-family, expired,
  expire, persist, append, setrange, incr paths. For partial mutations (append/setrange/incr)
  the value shipped is the resulting value, not the operand.

## Impact

- Affected specs: specs/kv-watch/spec.md (ADDED)
- Affected code: crates/synap-core/src/core/keyspace.rs (or new core/watch.rs),
  crates/synap-core/src/core/kv_store/store.rs, crates/synap-core/src/core/pubsub.rs (reuse only)
- Breaking change: NO
- User benefit: foundation for `kv.watch()` — real-time key change broadcast with values,
  reusing the proven pub/sub fan-out and backpressure.
