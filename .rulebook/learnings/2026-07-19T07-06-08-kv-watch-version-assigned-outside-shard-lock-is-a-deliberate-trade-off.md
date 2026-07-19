# KV watch: version assigned outside shard lock is a deliberate trade-off
**Source**: manual
**Date**: 2026-07-19
**Related Task**: phase21_kv-watch-core-notifier
**Tags**: kv-watch, pubsub, concurrency, performance
KeyWatchNotifier bumps the per-key version after the shard lock drops, so concurrent writers on the same key can publish watch events out of order (higher version != later value). Fixing it would move the PubSubRouter has_subscriber lookup inside every shard write lock, coupling all shards through router locks on the hot mutation path — wrong trade for a best-effort layer. Decision: keep lock-free notify, document version as gap/staleness signal only (docs/kv-watch.md). Also: version counters live in the notifier map (not StoredValue) so memory is bounded by the watched keyspace; versions reset after del/expired by design.