## 1. Implementation
- [x] 1.1 `KeyWatchNotifier` in `crates/synap-core/src/core/watch.rs`, publishing `__watch@0__:<key>` through `PubSubRouter`, always-on once attached. The envelope is the `WatchEvent` struct; it is serialized as `serde_json::Value` because that is what `PubSubRouter::publish` takes — the MessagePack encoding happens at the SynapRPC boundary, so the envelope reaches a Thunder client as MessagePack without watch needing its own codec
- [x] 1.2 Per-key counter starting at 1, `DEFAULT_INLINE_VALUE_CAP` = 64 KiB with `with_inline_cap` to override. A non-UTF-8 value also degrades to notify-only rather than being lossily re-encoded, which would hand the watcher bytes the key does not hold
- [x] 1.3 New `PubSubRouter::has_subscriber` — allocation-free, unlike the delivery path which clones the subscriber set. Checked before the version bump too, so an unwatched key does not grow the counter map
- [x] 1.4 Wired at set, getset and del. The key clone that feeds the notification was gated on the keyspace notifier alone; since keyspace defaults off and watch does not, that gate now fires for either
- [x] 1.5 Wired at expired, expire, persist, append, setrange and incr/decr. append and setrange capture the merged bytes inside the shard lock — only when a notifier is attached, so the copy stays off the common path — and incr renders the resulting total
- [x] 1.6 `cargo check`, `cargo clippy --workspace --all-targets` (clean) and `cargo fmt --all`

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 `docs/kv-watch.md` — why it is not keyspace notifications, the envelope, the best-effort/version semantics, the two notify-only degradations, the idle cost, and how to attach it
- [x] 2.2 20 tests: 12 unit in `watch.rs` (channel name, idle no-op, counter-map bound, versioning, cap boundary, non-UTF-8, envelope field omission) and 8 integration in `store_tests.rs` driving real mutations through a subscribed router
- [x] 2.3 423 passed, 0 failed across synap-core
