# Proposal: phase8_v1-release-1.0.0

Source: docs/analysis/synap-v1-release/ (closes the release; consolidates F-001..F-019)

## Why
Phases 1–7 leave the workspace restructured into `crates/synap-{core,protocol,server,cli,migrate}`,
dependencies current, stability hardened, and Redis benchmarks published — but the version
still reads `1.0.0-rc.1`, the CHANGELOG has no migration guide for the breaking crate split,
and user-facing docs still describe the old flat layout. A 1.0.0 is a public promise; the
final phase makes the release coherent: version, migration notes, docs, tag.

## What Changes
1. `[workspace.package] version = "1.0.0"` (all member crates inherit).
2. CHANGELOG 1.0.0 entry with an explicit migration guide: Rust consumers importing
   `synap_server::core::*` / `synap_server::protocol::*` move to `synap_core::*` /
   `synap_protocol::*`; note the umbrella re-exports kept for transition (mirroring
   Vectorizer's facade note); note the SynapRPC bind-default change from phase 6.
3. Docs sweep: `README.md`, `AGENTS.override.md` (workspace tree + module layers),
   `DOCKER_README.md`, `docs/` architecture pages, helm chart `appVersion`.
4. Repo hygiene: delete the stray `tatus --short` file at repo root (accidental shell
   redirect artifact) — explicit user authorization for this deletion is part of approving
   this task.
5. Verify the full release gate on `release/v1.0.0`: check → clippy `-D warnings` →
   fmt --check → full test suite → benches compile → Docker image builds.
6. Tag `v1.0.0` and open the release PR from `release/v1.0.0` to `main`.

## Impact
- Affected specs: none new (consolidation)
- Affected code: root `Cargo.toml`, `CHANGELOG.md`, `README.md`, `AGENTS.override.md`,
  `DOCKER_README.md`, `helm/`, docs pages
- Breaking change: YES — crate split changes Rust import paths (documented migration guide);
  SynapRPC default bind change (one-line config migration)
- User benefit: coherent, documented 1.0.0 with a credible stability and performance story
