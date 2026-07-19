## 1. Implementation
- [ ] 1.1 Create `KeyWatchNotifier` in synap-core publishing `__watch@0__:<key>` via `PubSubRouter`, always-on, with MessagePack envelope `{ key, event, version, value?, truncated? }`
- [ ] 1.2 Implement per-key monotonic `version` counter and inline-value cap with `truncated` degradation (default 64 KiB, constructor-configurable)
- [ ] 1.3 Implement no-subscriber fast path (bypass serialization and publish when the router has no match for the watch channel)
- [ ] 1.4 Wire `KeyWatchNotifier` into KV set/del/setex-family notify sites with post-mutation value
- [ ] 1.5 Wire expired/expire/persist/append/setrange/incr sites — partial mutations ship the resulting value
- [ ] 1.6 Run `cargo check` + `cargo clippy -- -D warnings` + `cargo fmt`

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior (envelope shape, versioning, cap degradation, no-subscriber no-op, post-mutation value on append/incr)
- [ ] 2.3 Run tests and confirm they pass
