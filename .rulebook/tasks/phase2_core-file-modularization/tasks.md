## 1. Implementation
- [ ] 1.1 Split `core/list.rs` into a `list/` module (extract `#[cfg(test)]` to `tests.rs`, then operation groups); preserve public paths via `pub use`
- [ ] 1.2 Split `core/sorted_set.rs` into a `sorted_set/` module the same way
- [ ] 1.3 Split `core/queue.rs` into a `queue/` module the same way
- [ ] 1.4 Move the large per-family handlers out of `protocol/resp3/command/mod.rs` into its sibling command modules; leave only dispatch in `mod.rs`
- [ ] 1.5 Extract the inline test module (and any self-contained helper groups) out of `kv_store/store.rs`

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation (AGENTS.override.md module-layer notes if any path label changed)
- [ ] 2.2 Write tests covering the new behavior (none new — this is behavior-preserving; the existing suite is the regression guard and must pass unchanged)
- [ ] 2.3 Run tests and confirm they pass (after EACH file: `cargo check` + `clippy -D warnings` + full tests, green before the next file)
