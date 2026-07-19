# Per-site notification wiring misses parallel write paths

**Category**: core
**Tags**: kv-store, notifications, kv-watch, keyspace

## Description

KVStore notifications (keyspace + watch) are wired call-site by call-site. Auditing after the watch feature found four silent paths: set_with_opts (the server's primary SET handler path) published nothing at all, lazy expiration on read skipped the expired event, eviction never fired (EventClass::Evicted existed unused), and FLUSHDB left watch version counters alive. When adding a notification concern to KVStore, grep every fn that inserts/removes from shard data — not just the sites that already call notify_* — and pair each removal path with the terminal event + counter cleanup. Fixed in 98f7fde with regression tests per path.
