# KV watch fan-out is linear at ~1µs/watcher; idle is ~115ns
**Source**: manual
**Date**: 2026-07-19
**Related Task**: phase26_kv-watch-interop-bench-docs
**Tags**: kv-watch, performance, benchmark, pubsub, phase26
benches/kv_watch_bench.rs measured KeyWatchNotifier::notify on the shared PubSubRouter: unwatched key ~115ns (stops at has_subscriber, no envelope/version), 1 watcher ~2.1µs, 10 ~9µs, 100 ~94µs, 1000 ~1.0ms — linear at roughly 1µs per watcher, with a wildcard subscription costing the same as an exact one. A stalled watcher (bounded buffer never drained) does not slow delivery to healthy ones: the router try_sends and drops it. Practical ceiling: fan-out cost is paid on the mutating command's thread after the shard lock drops, so a key with thousands of watchers adds ~1ms to that write path — shard the key or use notify mode if that matters. Numbers are in docs/kv-watch.md.