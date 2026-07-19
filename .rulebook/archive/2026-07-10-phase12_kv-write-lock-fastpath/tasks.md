## 1. Implementation
- [x] 1.1 Change KeyLockManager from per-shard tokio::Mutex to per-shard RwLock
- [x] 1.2 Plain writers (set/incr/delete/getset) take read_key; EXEC takes write_keys
- [x] 1.3 Verify M-010 isolation preserved (write excludes read both directions)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
