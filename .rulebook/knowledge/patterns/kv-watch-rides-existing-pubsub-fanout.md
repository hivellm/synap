# KV watch rides the existing pub/sub fan-out — do not build a new engine

**Category**: architecture
**Tags**: analysis:kv-watch-observable, pubsub, watch, kv, thunder

## Description

For broadcast-on-key-update (observable KV), the correct shape is a thin composition layer
over infrastructure that already exists, not a new subsystem. Publish a value-carrying
envelope `{ key, event, version, value?, truncated? }` to a dedicated **always-on**
`__watch@0__:<key>` channel family through the existing `PubSubRouter`
(trie matching + slow-consumer drop, `crates/synap-core/src/core/pubsub.rs:443-483`) and
deliver via the existing Thunder RPC push bridge
(`crates/synap-server/src/protocol/synap_rpc/server.rs:122-199`). That inherits a
battle-tested fan-out, backpressure, and wildcard story that already scales to massive
connection counts.

Do **not** reuse the Redis-compat `__keyspace@` channels: they are gated by
`notify_keyspace_events` (default OFF, so watch would silently break at default config)
and cannot carry values without breaking Redis semantics. The post-mutation value is
already in scope at every notify site in `kv_store/store.rs` — it just was never forwarded.

Semantics: best-effort with per-key monotonic `version` for gap detection; inline-value cap
with notify-only degradation for large values; replay/at-least-once belongs to streams.

## Example

// Notify site already has the value in scope (kv_store/store.rs:372-374):
// self.notify_keyspace(EventClass::String, "set", &k);   // event name only — the gap
// watch path forwards it:
// self.watch_notifier.notify(&k, WatchEvent::Set, Some(&value));

## When to Use

Any "observe X for changes" feature (KV watch, config watch, cache invalidation) — check
for an existing fan-out + push path before designing a new one. Full analysis:
`docs/analysis/kv-watch-observable/`.

## When Not To Use

When subscribers need guaranteed/replayable delivery — that is the streams subsystem
(`core/stream.rs`), a deliberately heavier design.
