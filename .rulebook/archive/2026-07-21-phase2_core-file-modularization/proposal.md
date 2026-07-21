# Proposal: phase2_core-file-modularization

## Why

Five source files carry most of the core data-structure logic in single monolith
files, each mixing the public type, its many operations, and a large inline
`#[cfg(test)]` module:

- `crates/synap-core/src/core/list.rs` — 1844 lines
- `crates/synap-core/src/core/kv_store/store.rs` — 1804 lines
- `crates/synap-core/src/core/sorted_set.rs` — 1492 lines
- `crates/synap-core/src/core/queue.rs` — 1422 lines
- `crates/synap-server/src/protocol/resp3/command/mod.rs` — 1286 lines

The project already set the precedent that oversized files should be split —
the former 11K-line `server/handlers.rs` is now a `server/handlers/` directory,
and `kv_store` is already a folder. Files this large slow every edit: the LLM
editing discipline in AGENTS.md (`sequential-editing`, `task-decomposition`)
explicitly loses accuracy past a few hundred lines, and reviewers cannot hold
1800 lines in working memory. Nothing here is a behavior change — this is a
pure, mechanical, behavior-preserving reorganization gated by the full test
suite.

## What Changes

For each file, extract cohesive operation groups into sibling submodules under a
folder of the same name, keeping the public type and its `impl` re-exported so
**no call site outside the module changes**:

- `list.rs` → `list/{mod,ops,blocking,tests}.rs` (mutations, blocking pops, tests)
- `sorted_set.rs` → `sorted_set/{mod,range,rank,tests}.rs`
- `queue.rs` → `queue/{mod,ack,delivery,tests}.rs`
- `resp3/command/mod.rs` → move the large per-family command handlers into the
  already-existing sibling command modules; `mod.rs` keeps only dispatch.
- `kv_store/store.rs` → split the inline test module and any self-contained
  helper groups out of `store.rs` (it already lives in a folder).

Each split moves the inline `#[cfg(test)]` block into its own `tests.rs` file
first (biggest, safest win), then peels off operation groups. `cargo check` +
`clippy -D warnings` + full tests run after **each** file so a regression is
localized. Public API and module paths are preserved via `pub use`.

## Impact
- Affected specs: none (no behavior or spec change)
- Affected code: the five files above and their new sibling submodules;
  `mod.rs` re-exports only
- Breaking change: NO — internal reorganization, public paths preserved
- User benefit: faster, safer edits and reviews on the hottest core files;
  smaller compilation units for incremental rebuilds
