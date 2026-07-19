## 1. Implementation
- [x] 1.1 Add `WATCH`/`UNWATCH` commands to the SynapRPC dispatch layer, mapping to subscribe/unsubscribe on `__watch@0__:<pattern>` over the existing push bridge — shipped as `KV.WATCH`/`KV.UNWATCH` because plain `WATCH` is the transaction command and the arg shapes (`WATCH client_id key` vs `WATCH pattern mode`) are indistinguishable
- [x] 1.2 Implement per-subscription `mode` (`value` default, `notify` strips inline value before push) — strip happens in the push bridge (`strip_watch_value`), dropping `value` and `truncated`
- [x] 1.3 Complete `kv_websocket`: replace the 501 stub mirroring `handle_pubsub_socket`, honoring `?keys=` with wildcard support, JSON envelope frames — reuses `handle_pubsub_socket` directly, keys map to `__watch@0__:<key>` channels. Wildcard keys required a new in-segment `Glob` matcher in `PubSubRouter` (F-011 assumed the existing matcher covered `user:*`, but `*` only matched whole `.`-segments)
- [x] 1.4 Add `watch.max_inline_value_bytes` config (default 65536) + env override, threaded into the notifier — `WatchConfig` + `SYNAP_WATCH_MAX_INLINE_VALUE_BYTES`, notifier attached in all three KVStore construction paths in main.rs
- [x] 1.5 Run `cargo check` + `cargo clippy -- -D warnings` + `cargo fmt` — workspace clippy clean, all targets

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation (protocol doc for WATCH/UNWATCH + /kv/ws) — docs/kv-watch.md gained SynapRPC/WebSocket/Configuration sections, config.example.yml gained the watch section, CHANGELOG updated
- [x] 2.2 Write tests covering the new behavior (RPC watch end-to-end set→push, notify mode, WS watch, wildcard watch, unwatch stops delivery) — 4 glob matcher/router tests, 5 dispatch tests, 1 strip test, 3 WebSocket integration tests
- [x] 2.3 Run tests and confirm they pass — 433 green in synap-core, full synap-server suite green
