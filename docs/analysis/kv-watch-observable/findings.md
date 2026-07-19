# Findings — kv-watch-observable

Numbered findings with evidence. Confidence: High = verified at file:line; Medium = design
choice or needs per-item triage during implementation.

## Workstream A — Server infrastructure (what already exists)

### F-001 — Server-push pub/sub over Thunder RPC is fully implemented and is the right transport
- Evidence: `crates/synap-server/src/protocol/synap_rpc/server.rs:122-199` — after a `SUBSCRIBE`
  command succeeds, `bridge_subscription` registers the connection's push channel with the
  `PubSubRouter`; published messages are delivered as Thunder push frames (`id == PUSH_ID`)
  via a bounded `mpsc` channel (`SUBSCRIBER_CHANNEL_CAPACITY = 1024`).
  `config.rs:14,34` marks Synap as "the family's one shipping push producer."
- Consequence: a true async push path (not polling) already exists — exactly what a
  million-connection watch needs. No new transport work required.
- Impact: HIGH (reuse). Confidence: High.

### F-002 — Keyspace notifications fire on every KV mutation, but carry event-name only (no value)
- Evidence: `crates/synap-core/src/core/keyspace.rs` — `KeyspaceNotifier` publishes to
  `__keyspace@0__:<key>` (payload = event name, e.g. `"set"`) and `__keyevent@0__:<event>`
  (payload = key) through the `PubSubRouter`. `kv_store/store.rs:372-374` calls
  `notify_keyspace(EventClass::String, "set", &k)` after the shard lock drops.
- Gap: the new value (`value: Arc<[u8]>`) is in scope at the notify site but is **not
  forwarded** — the notifier signature is `notify(class, event, key)` with no value.
- Consequence: this is the single real server-side gap for a value-broadcasting watch.
- Impact: HIGH (core gap). Confidence: High.

### F-003 — The `/kv/ws` WATCH WebSocket endpoint is a registered 501 stub
- Evidence: `crates/synap-server/src/server/handlers/websocket.rs:7-37` — `kv_websocket`
  parses `?keys=` then returns `501 NOT_IMPLEMENTED` with "would require KVStore to support
  change notifications." Route wired at `server/router.rs:121` (`// WebSocket for WATCH (future)`).
- Consequence: the stub's stated precondition is now satisfied by the keyspace notifier
  (F-002); the handler can be completed by mirroring `handle_pubsub_socket`
  (`websocket.rs:431-561`), which is a correct push-based reference implementation.
- Impact: MEDIUM. Confidence: High.

### F-004 — `PubSubRouter` already has slow-consumer backpressure; it is the fan-out reuse point
- Evidence: `crates/synap-core/src/core/pubsub.rs:443-483` — `deliver_message` uses
  non-blocking `try_send`; a subscriber whose bounded buffer fills is disconnected and
  counted in `slow_consumers_dropped` (audit M-011). Trie-based exact matching plus a
  separate wildcard list.
- Consequence: reusing `PubSubRouter` for KV watch inherits a battle-tested scaling and
  backpressure story; no new fan-out engine is needed.
- Impact: HIGH (reuse). Confidence: High.

### F-005 — Notify sites already cover the full KV mutation surface
- Evidence: `kv_store/store.rs` fires notifications at: set (373, 1645), del (661),
  generic setex/etc (763), expired (1153), expire (1385), persist (1404), append (1473),
  setrange (1577). Collection stores (hash/list/set/sorted_set) also fire
  (`keyspace.rs:239-285` tests confirm).
- Caveat: append/setrange/incr are partial mutations — the value shipped to watchers must
  be the *post-mutation* value, otherwise watchers must re-GET (extra round-trip, race window).
- Impact: MEDIUM. Confidence: High.

## Workstream B — Design decisions

### F-006 — Keyspace notifications are opt-in and default OFF; watch needs its own always-on channel
- Evidence: `crates/synap-server/src/config.rs:84-91` + `main.rs:257-268` —
  `notify_keyspace_events` defaults to empty, so the notifier is `None` and every notify
  site is a single no-op branch.
- Decision: reusing `__keyspace@` channels would couple watch to the Redis-compat flag
  (watch would silently break at default config) and cannot carry values without breaking
  Redis semantics. **Chosen: a dedicated always-on `__watch@0__:<key>` channel family that
  carries the post-mutation value**, published through the same `PubSubRouter`, independent
  of `notify_keyspace_events`. Watch publishes are only produced when at least one
  subscriber matches (router-side check), so idle cost at default config stays near zero.
- Impact: HIGH (design). Confidence: Medium (design choice, validated against code).

### F-007 — Delivery is best-effort by design; replay belongs to streams, not watch
- Evidence: pub/sub slow-consumer drop (F-004); at-least-once/replay semantics live in the
  streams subsystem (`core/stream.rs`), a different and heavier design.
- Decision: `watch` is a best-effort, latest-value notification primitive. A stalled watcher
  is disconnected (existing behavior) and must re-GET + re-subscribe. Watch events include a
  per-key monotonic `version` so clients can detect gaps. Users needing replay use streams.
- Impact: MEDIUM (documented semantics). Confidence: High.

### F-008 — Large-value fan-out needs a notify-only mode
- Evidence: value-inline broadcast to N watchers multiplies bandwidth by value size × N.
- Decision: watch supports two modes per subscription — `value` (default, payload inline)
  and `notify` (event + key + version only; client re-GETs on demand). Server caps inline
  payloads (configurable, default 64 KiB) and degrades to `notify` above the cap, flagged
  in the event envelope.
- Impact: MEDIUM. Confidence: Medium (design choice).

## Workstream C — SDK surface

### F-009 — All six SDKs already implement the push-subscribe primitive
- Evidence: `subscribePush`/equivalent exists in: `sdks/typescript/src/transports/synap-rpc.ts`
  (used by `pubsub.ts:105-122`), `sdks/python/synap_sdk/transport_rpc.py` + `modules/pubsub.py`,
  `sdks/php/src/SynapRpcTransport.php` + `Module/PubSubManager.php`,
  `sdks/csharp/src/Synap.SDK/SynapRpcTransport.cs` + `Modules/PubSubManager.cs`,
  `sdks/rust/src/transport/mod.rs` + `pubsub_reactive.rs`, and `sdks/go/pubsub.go` +
  `transport_rpc.go`.
- Consequence: `kv.watch(key)` in each SDK is a thin wrapper over the existing
  subscribe-push call targeting the per-key watch channel. Go IS in scope ("todas as SDKs").
- Impact: HIGH (reuse). Confidence: High.

### F-010 — Reactive/Observable scaffolding already exists; watch must return the native reactive type
- Evidence: Rust SDK has a full `rx` module (`sdks/rust/src/rx/{observable,subject,operators}.rs`,
  RxJS-parity per README) plus `pubsub_reactive.rs`, `queue_reactive.rs`, `stream_reactive.rs`.
  TS SDK returns `rxjs` `Observable<ProcessedPubSubMessage<T>>` (`pubsub.ts`). Python/PHP/C#/Go
  expose async-iterator / callback / channel styles in their `PubSub` managers.
- Decision: `kv.watch(key)` returns each SDK's existing reactive idiom — `Observable` in
  Rust/TS, async iterator in Python, callback/iterator in PHP, `IAsyncEnumerable` in C#,
  channel in Go — for API consistency with pub/sub.
- Impact: MEDIUM. Confidence: High.

### F-011 — Wildcard/prefix watch is essentially free via the existing router
- Evidence: `PubSubRouter` already supports `*`/`#` wildcards (trie + wildcard list, F-004).
- Decision: expose `watch("user:*")` from day one — it costs nothing extra server-side and
  is a headline feature for cache-invalidation use cases.
- Impact: LOW (scope add, near-zero cost). Confidence: High.
