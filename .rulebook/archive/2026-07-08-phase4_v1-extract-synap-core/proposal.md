# Proposal: phase4_v1-extract-synap-core

Source: docs/analysis/synap-v1-release/ (F-002, F-004)

## Why
The data-structure engine (`core/` — 16,526 LOC across 23 files: kv_store, hash, list, set,
sorted_set, bitmap, hyperloglog, geospatial, queue, stream, pubsub, transaction, error) is
the stable heart of Synap and has no dependency on HTTP, auth, replication or the hub. In
Vectorizer and Nexus this layer lives in `<name>-core`, which shortens incremental builds
(server changes no longer recompile the engine), makes the engine independently testable
and benchmarkable, and enforces the Foundation → Core → Features → Presentation DAG that
the project rules mandate. `cache/`, `compression/` and `simd/` are engine-support modules
with the same leaf profile and move together.

## What Changes
1. Create leaf crate `crates/synap-core` ← `core/`, `cache/`, `compression/`, `simd/`.
   Its `Cargo.toml` depends only on engine-level crates (tokio, parking_lot, radix_trie,
   serde, compact_str, lz4, zstd, thiserror…) — never on `synap-server` or `synap-protocol`.
2. Rewrite imports across the server: `crate::core::X` → `synap_core::X` (same for cache/
   compression/simd). Mechanical but touches nearly every server file — executed one module
   at a time, one commit per module, `cargo check` after each (fail-twice-escalate applies).
3. `AppState` (`server/handlers/mod.rs`) and the dispatch layers rehomed in phase 3 now
   compile against `synap-core` + `synap-protocol`.
4. `SynapError` stays in `synap-core` (`core/error.rs`) as the shared error type; server-only
   error variants, if any, get a server-side wrapper rather than polluting the core type.

Gate: `cargo check --workspace` → `clippy -D warnings` → `fmt --check` → `cargo test`
(engine unit tests move with the code; integration tests stay in synap-server).

## Impact
- Affected specs: none (code location only, no behavior change)
- Affected code: new `crates/synap-core/`; `crates/synap-server/src/**` import rewrites;
  `benches/` targets pointed at the core crate where applicable
- Breaking change: NO at runtime; YES for Rust consumers importing `synap_server::core::*`
  (migration note lands in phase 8 CHANGELOG)
- User benefit: faster incremental builds, independently benchmarkable engine, enforced
  layering that prevents future server→core entanglement
