## 1. Implementation
- [x] 1.1 Register the replica in the broadcast set before taking the snapshot
- [x] 1.2 After full/partial sync, live ops buffered during transfer are streamed (fixed partial-sync framing bug too)
- [x] 1.3 Replica dedups by offset (ignores ops the snapshot already covers) so overlap is harmless
- [x] 1.4 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (replication.md join semantics)
- [x] 2.2 Write tests covering the new behavior (test_concurrent_writes_during_sync re-enabled, asserts 500 keys)
- [x] 2.3 Run tests and confirm they pass
