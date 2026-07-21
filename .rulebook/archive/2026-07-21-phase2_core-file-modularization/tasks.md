## 1. Implementation
- [x] 1.1 Split `core/list.rs` into a `list/` module — mod (ListValue) + store (ListStore); public paths preserved via `pub use` (commit 0a60f21)
- [x] 1.2 Split `core/sorted_set.rs` into a `sorted_set/` module — mod (value+helpers) + store (commit 27004c4)
- [x] 1.3 Split `core/queue.rs` into a `queue/` module — mod (Queue) + manager (QueueManager) + tests (commit 3428467)
- [x] 1.4 `protocol/resp3/command/mod.rs`: per-family handlers already live in sibling modules; extracted the inline test module to `command/tests.rs`, leaving only dispatch + decls in mod.rs (commit b9281de)
- [x] 1.5 `kv_store/store.rs`: inline tests were already in `store_tests.rs`; extracted the String-extension command family into a sibling `impl KVStore` (`store_string_ops.rs`), 1804 → 1520 lines

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation (no external path labels changed — public module paths preserved via `pub use`; each new file carries a module doc-comment explaining the split)
- [x] 2.2 Write tests covering the new behavior (none new — behavior-preserving; the existing suite is the regression guard and passed unchanged)
- [x] 2.3 Run tests and confirm they pass (per file: `cargo check` + `clippy -D warnings` + module tests — list 24, sorted_set 21, queue 21, resp3 command 69, kv_store 61, all green)
