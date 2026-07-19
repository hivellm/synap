# Proposal: phase2_v1-workspace-skeleton

Source: docs/analysis/synap-v1-release/ (F-001, F-002)

## Why
Synap is the last of the three sibling stores (Vectorizer, Nexus, Synap) still on a flat
workspace layout (`members = ["synap-server", "synap-cli", "synap-migrate", "sdks/rust"]`).
Vectorizer and Nexus already use `crates/<name>-{core,protocol,server,cli}` with full
`[workspace.package]`/`[workspace.dependencies]` inheritance, and Vectorizer's split
(archived task `phase4_split-vectorizer-workspace`) proved the safe migration order:
first a pure directory move with zero refactor, then crate extractions in later phases.
Doing the skeleton move as its own phase keeps the diff purely mechanical and reviewable.

## What Changes
On the `release/v1.0.0` branch:
1. `git mv synap-server crates/synap-server`, `git mv synap-cli crates/synap-cli`,
   `git mv synap-migrate crates/synap-migrate`. No source-code changes; all `use crate::X`
   paths stay valid.
2. Root `Cargo.toml`: `members = ["crates/*", "sdks/rust"]`; extend
   `[workspace.package]` / `[workspace.dependencies]` / `[workspace.lints]` following the
   Nexus pattern so member crates inherit shared fields and dep versions.
3. Update every path that references the old locations: CI workflows (`.github/`),
   `Dockerfile`, `docker-compose.yml`, `helm/`, `scripts/`, benchmark invocations, and docs
   that show the workspace tree (`AGENTS.override.md`, `README.md`, `DOCKER_README.md`).

Gate: `cargo check --workspace` → `cargo clippy -- -D warnings` → `cargo fmt --check` →
`cargo test` all green before the phase closes.

## Impact
- Affected specs: none (structure only, no behavior change)
- Affected code: directory layout of `synap-server/`, `synap-cli/`, `synap-migrate/`;
  root `Cargo.toml`; `.github/workflows/`; `Dockerfile`; `helm/`; `scripts/`
- Breaking change: NO at runtime; YES for local tooling that hardcodes old paths (updated here)
- User benefit: workspace organized like Vectorizer/Nexus, enabling the crate extractions
  in phases 3–5 and making cross-project navigation consistent
