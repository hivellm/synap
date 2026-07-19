# Execution plan — kv-watch-observable

Six sequential rulebook tasks (phase21..phase26). Each task is gated by
`cargo check` / language type-check + `clippy -D warnings` / lint + full tests before commit.

## Phase 21 — `phase21_kv-watch-core-notifier`  (findings F-002, F-004, F-005, F-006, F-007, F-008)

Server core: value-carrying watch notifier in `synap-core`.

- New `KeyWatchNotifier` alongside `KeyspaceNotifier` in `core/keyspace.rs` (or sibling
  `core/watch.rs`): publishes to `__watch@0__:<key>` via the existing `PubSubRouter`,
  **always-on**, independent of `notify_keyspace_events`.
- Event envelope (MessagePack): `{ key, event, version, value?, truncated? }` — `version` is a
  per-key monotonic counter; `value` omitted in notify-only degradation (> configurable cap,
  default 64 KiB).
- Cheap idle path: skip serialization/publish when the router has no matching subscriber.
- Thread the post-mutation value through every KV notify site (set, del, setex, expire(d),
  persist, append, setrange, incr paths) — post-mutation value, not the operand.
- Unit tests in `synap-core` for envelope, versioning, cap degradation, no-subscriber no-op.

## Phase 22 — `phase22_kv-watch-server-endpoints`  (findings F-001, F-003)

Server surface: expose watch over SynapRPC and WebSocket.

- SynapRPC: `WATCH`/`UNWATCH` commands in the dispatch layer that resolve to
  SUBSCRIBE/UNSUBSCRIBE on `__watch@0__:<key>` and ride the existing push bridge
  (`synap_rpc/server.rs:122-199`) — plus per-subscription mode flag (`value` | `notify`).
- Complete `/kv/ws` (`websocket.rs:7-37`): replace the 501 stub mirroring
  `handle_pubsub_socket` (431-561), honoring `?keys=` (comma list, wildcard allowed).
- Config: `watch.max_inline_value_bytes` (default 65536), wired via config.yml + env override.
- Integration tests: RPC watch end-to-end (set → push received), WS watch, wildcard watch,
  slow-consumer drop still applies.

## Phase 23 — `phase23_kv-watch-sdk-rust-ts`  (findings F-009, F-010, F-011)

- Rust SDK: `kv.watch(pattern) -> Observable<WatchEvent>` via `rx` module, mirroring
  `pubsub_reactive.rs`; typed `WatchEvent { key, event, version, value }`.
- TypeScript SDK: `kv.watch(pattern): Observable<WatchEvent<T>>` via rxjs, mirroring
  `pubsub.ts`; automatic re-GET helper for notify-only events.
- Tests + README examples in both SDKs.

## Phase 24 — `phase24_kv-watch-sdk-python-php`  (findings F-009, F-010)

- Python SDK: `async for event in kv.watch(pattern)` async-iterator, mirroring
  `modules/pubsub.py`.
- PHP SDK: callback/iterator API mirroring `Module/PubSubManager.php`.
- Tests + README examples in both SDKs.

## Phase 25 — `phase25_kv-watch-sdk-csharp-go`  (findings F-009, F-010)

- C# SDK: `IAsyncEnumerable<WatchEvent>` mirroring `Modules/PubSubManager.cs`.
- Go SDK: channel-based `kv.Watch(ctx, pattern) (<-chan WatchEvent, error)` mirroring
  `pubsub.go`.
- Tests + README examples in both SDKs.

## Phase 26 — `phase26_kv-watch-interop-bench-docs`  (all findings)

- Cross-SDK interop: set from each SDK, observe from every other (s2s-tests feature).
- Fan-out benchmark: N watchers on one key, publish latency/throughput profile; document
  the slow-consumer semantics and the notify-only cap.
- Docs: protocol doc for the `__watch@0__` channel + envelope, per-SDK README sections,
  CHANGELOG entries.

## Risk register

| Risk | Mitigation |
|---|---|
| Partial-mutation ops ship wrong value | Thread the post-mutation value explicitly per site; test append/setrange/incr (F-005) |
| Watch silently off at default config | Dedicated always-on channel, never gated by `notify_keyspace_events` (F-006) |
| Bandwidth blow-up on large values | Inline cap + notify-only degradation, flagged in envelope (F-008) |
| Watchers assume guaranteed delivery | Document best-effort + `version` gap detection; point replay users to streams (F-007) |
| Go SDK left inconsistent | Explicitly in scope, phase25 (F-009) |
