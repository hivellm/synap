## 1. Implementation
- [ ] 1.1 Add `WATCH`/`UNWATCH` commands to the SynapRPC dispatch layer, mapping to subscribe/unsubscribe on `__watch@0__:<pattern>` over the existing push bridge
- [ ] 1.2 Implement per-subscription `mode` (`value` default, `notify` strips inline value before push)
- [ ] 1.3 Complete `kv_websocket`: replace the 501 stub mirroring `handle_pubsub_socket`, honoring `?keys=` with wildcard support, JSON envelope frames
- [ ] 1.4 Add `watch.max_inline_value_bytes` config (default 65536) + env override, threaded into the notifier
- [ ] 1.5 Run `cargo check` + `cargo clippy -- -D warnings` + `cargo fmt`

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation (protocol doc for WATCH/UNWATCH + /kv/ws)
- [ ] 2.2 Write tests covering the new behavior (RPC watch end-to-end set→push, notify mode, WS watch, wildcard watch, unwatch stops delivery)
- [ ] 2.3 Run tests and confirm they pass
