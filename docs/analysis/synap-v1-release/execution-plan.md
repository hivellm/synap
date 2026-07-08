# Execution Plan — synap-v1-release

All work happens on a `release/v1.0.0` branch cut from `main` in phase 1.
Every phase gates on: `cargo check --workspace` → `cargo clippy -- -D warnings` →
`cargo fmt --check` → `cargo test` (diagnostic-first). Phases map 1:1 to rulebook tasks
and MUST execute in order (the list is an order, not a menu).

## Phase 1 — Dependabot + version baseline (`phase1_v1-dependabot-and-version-baseline`)

Pre-flight on `main` (these merges land before the release branch is cut):

1. Batch-merge the 5 TS-SDK dev-dep PRs (#225-#229) after one combined CI pass (F-009).
2. Merge the 3 Cargo PRs individually, lowest-risk first, each with its own compile+test gate:
   sysinfo 0.39 (#222) → mlua 0.12 (#223) → rmcp 2.1 (#224, two-major jump — keep 1.5 fallback
   until MCP tests are green) (F-008).
3. Prune superseded dependabot branches on origin (F-010).
4. Fix version drift: `[workspace.package] version = "1.0.0-rc.1"`, convert `synap-server` and
   `sdks/rust` to `version.workspace = true` / `edition.workspace = true` (F-006).
5. Cut `release/v1.0.0` from the updated `main`.

## Phase 2 — Workspace skeleton, zero refactor (`phase2_v1-workspace-skeleton`)

Mirror Vectorizer sub-phase 1 (F-001, F-002):

- `git mv synap-server crates/synap-server`, same for `synap-cli`, `synap-migrate`.
- Root `Cargo.toml`: `members = ["crates/*", "sdks/rust"]`; adopt full `[workspace.package]` /
  `[workspace.dependencies]` / `[workspace.lints]` inheritance (Nexus pattern).
- No `use crate::X` changes. Update CI paths, Dockerfile, helm, scripts referencing old paths.

## Phase 3 — Extract `synap-protocol`, pure wire only (`phase3_v1-extract-synap-protocol`)

- New `crates/synap-protocol` with: `protocol/envelope.rs`, `resp3/parser.rs`, `resp3/writer.rs`,
  RESP value type, `synap_rpc/codec.rs`, `synap_rpc/types.rs`.
- **Guardrail (F-003):** `resp3/command/` and `synap_rpc/dispatch/` stay in `synap-server` —
  they depend on `AppState`/`core`/`scripting`; moving them creates a cycle.
- `synap-server` depends on `synap-protocol`.

## Phase 4 — Extract `synap-core` (`phase4_v1-extract-synap-core`)

- New leaf crate `crates/synap-core` ← `core/`, `cache/`, `compression/`, `simd/` (F-002).
- Rewrite `crate::core::X` → `synap_core::X` across the server (mechanical, one commit per module,
  `cargo check` after each).
- `AppState` and the dispatch layers compile against `synap-core` + `synap-protocol` (F-004).

## Phase 5 — Wire `sdks/rust` to `synap-protocol` (`phase5_v1-wire-rust-sdk-to-protocol`)

- Per-type diff of SDK wire types vs `synap-protocol` types; replace duplicates where shapes match (F-005).
- SDK inherits `[workspace.package]` fields (F-006 leftover).

## Phase 6 — Stability hardening (`phase6_v1-stability-hardening`)

- Flaky-test audit: run suite N times, log flakes; replace `sleep`-based sync with
  barriers/`Notify`/polling-with-deadline (F-011).
- Hot-path `unwrap()/expect()` triage in core/server non-test code → `?` + typed errors (F-012).
- Fix `Resp3Config::enabled` serde/Default contradiction (F-013).
- Unify listener bind defaults (both loopback by default; document opt-in exposure) (F-014).
- Burn down the 13 TODO/FIXME markers (F-015).

## Phase 7 — Redis benchmark + performance (`phase7_v1-redis-benchmark-and-perf`)

- Reconcile/annotate the old `synap-vs-redis` docs (mark resolved items) (F-016, F-019).
- Publish `redis-benchmark` numbers: Synap RESP3 vs Redis 7 (GET/SET/INCR/LPUSH/pipelined) (F-017).
- Verify pipelining depth in `resp3/server.rs`.
- Cheap wins: allocator feature flag (mimalloc), parallel MGET/MSET by shard (F-018).
- Decide + document per remaining parity item (BLPOP/BRPOP/BZPOPMIN, PSUBSCRIBE, keyspace
  notifications, SCAN cursors, LFU, IO threads): ship in 1.0 or documented post-1.0.

## Phase 8 — Release 1.0.0 (`phase8_v1-release-1.0.0`)

- `[workspace.package] version = "1.0.0"`.
- CHANGELOG with migration notes — the crate split is a breaking change for Rust consumers
  importing `synap_server::*` (mirror Vectorizer's umbrella-facade note).
- Docs sweep (README, AGENTS.override.md workspace tree, DOCKER_README, helm values), tag `v1.0.0`.
- Repo hygiene: remove the stray `tatus --short` file at repo root (accidental shell redirect).

## Risk register

| Risk | Mitigation |
|------|------------|
| rmcp 1.5→2.1 API churn (highest) | Own compile+test gate; keep 1.5 fallback branch until MCP tests green |
| Accidentally moving dispatch into `synap-protocol` → dependency cycle | Phase 3 guardrail; `cargo check` catches immediately |
| Import-rewrite volume in phase 4 (~every server file) | One commit per module, `cargo check` after each |
| Breaking Rust consumers of `synap_server::*` | Umbrella re-exports + CHANGELOG migration guide (phase 8) |
| Flaky tests masking real regressions during the split | Phase 6 ordered before benchmark/release phases |

Estimated total: ~9–10 weeks.
