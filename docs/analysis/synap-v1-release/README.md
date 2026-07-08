# Analysis: Synap v1.0.0 Release — Workspace Restructure, Stability & Performance

**Slug:** `synap-v1-release`
**Date:** 2026-07-08
**Status:** Approved — materialized into rulebook tasks
**Scope:**

1. Workspace restructure to a `crates/` layout following the Vectorizer/Nexus conventions
   (`crates/synap-{core,protocol,server,cli}`), on a `release/v1.0.0` branch.
2. Resolution of the open dependabot PRs as a pre-flight step.
3. Stability hardening for a 1.0 quality gate (flaky tests, hot-path `unwrap`s, config defaults).
4. Performance work and an updated architecture/feature comparison against Redis.

## Documents

- [findings.md](findings.md) — 19 numbered findings (F-001..F-019) across 4 workstreams, with evidence and confidence.
- [execution-plan.md](execution-plan.md) — 8 phases mapped 1:1 to rulebook tasks, plus risk register.

## Method

Direct source reading of the Synap workspace, the sibling `Vectorizer` and `Nexus` repos
(both already migrated to the target `crates/` shape — Vectorizer's split in
`phase4_split-vectorizer-workspace` explicitly cites Synap and Nexus as references),
`gh pr list` for the open PRs, and reconciliation against the prior
[`synap-vs-redis`](../synap-vs-redis/) analysis.

## Headline conclusions

1. **The restructure is well-precedented and mechanical** if done in incremental sub-phases
   (skeleton move → extract protocol → extract core → wire SDK), each gated by
   `cargo check` + `clippy -D warnings` + full tests.
2. **`protocol/` is not a pure wire module.** Its RESP3 command layer and SynapRPC dispatch
   layer call into `AppState`, `crate::core::*` and `crate::scripting`. Only
   parser/writer/codec/wire-types can move to `synap-protocol`; dispatch stays in
   `synap-server`, otherwise a dependency cycle is created (F-003).
3. **The prior `synap-vs-redis` analysis is partly outdated.** RESP3 (94-command dispatch,
   port 6379), SynapRPC (MessagePack, port 15501), eviction policies and the GET/SET hot-path
   fixes are already shipped. The v1.0 Redis story is "benchmark and tune", not "build" (F-016).
4. **Version drift must be fixed first:** `[workspace.package]` says 0.12.0 while 0.13.0 is
   already released; `synap-server` and `sdks/rust` hardcode versions instead of inheriting (F-006).

## Materialized rulebook tasks

| Task | Findings |
|------|----------|
| `phase1_v1-dependabot-and-version-baseline` | F-006..F-010 |
| `phase2_v1-workspace-skeleton` | F-001, F-002 |
| `phase3_v1-extract-synap-protocol` | F-003 |
| `phase4_v1-extract-synap-core` | F-002, F-004 |
| `phase5_v1-wire-rust-sdk-to-protocol` | F-005 |
| `phase6_v1-stability-hardening` | F-011..F-015 |
| `phase7_v1-redis-benchmark-and-perf` | F-016..F-019 |
| `phase8_v1-release-1.0.0` | all |
