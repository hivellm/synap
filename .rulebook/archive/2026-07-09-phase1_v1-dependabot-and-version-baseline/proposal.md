# Proposal: phase1_v1-dependabot-and-version-baseline

Source: docs/analysis/synap-v1-release/ (F-006, F-007, F-008, F-009, F-010)

## Why
The v1.0.0 restructure must start from a clean baseline. Today 8 dependabot PRs are open
(3 Cargo, 5 npm), three of them are breaking major bumps touching load-bearing subsystems
(rmcp → MCP layer, mlua → Lua scripting, sysinfo → process metrics), and the workspace
version is a full release behind reality (`[workspace.package] version = "0.12.0"` while
0.13.0 is already released; `synap-server` and `sdks/rust` hardcode version/edition instead
of inheriting). Cutting `release/v1.0.0` before resolving these would drag stale deps and
version drift into every subsequent phase.

## What Changes
1. Batch-merge the 5 TS-SDK dev-dep PRs (#225 vitest, #226 @vitest/coverage-v8,
   #227/#228/#229 typescript-eslint) after a combined CI pass — matches existing repo practice.
2. Merge the 3 Cargo PRs individually, lowest-risk first, each with its own
   `cargo check` + `cargo clippy -D warnings` + `cargo test` gate:
   sysinfo 0.33→0.39 (#222), then mlua 0.11.5→0.12.0 (#223), then rmcp 1.5.0→2.1.0 (#224 —
   two-major jump; every prior rmcp bump broke the API, keep the 1.5 state recoverable until
   MCP tests are green).
3. Prune superseded dependabot branches on origin (older eslint/msgpackr/vitest/typescript-eslint
   duplicates).
4. Fix version drift: set `[workspace.package] version = "1.0.0-rc.1"`, convert
   `synap-server/Cargo.toml` and `sdks/rust/Cargo.toml` to `version.workspace = true` /
   `edition.workspace = true`.
5. Cut the `release/v1.0.0` branch from the updated `main` (create the branch only; no
   checkout/switch of the shared working tree).

## Impact
- Affected specs: none (dependency + version baseline only)
- Affected code: `Cargo.toml` (root), `synap-server/Cargo.toml`, `sdks/rust/Cargo.toml`,
  `synap-server/src/server/mcp_*.rs` (rmcp API churn), `synap-server/src/scripting/` (mlua),
  `synap-server/src/server/metrics_handler.rs` (sysinfo), `sdks/typescript/package.json`
- Breaking change: NO (internal baseline; rc version pre-release)
- User benefit: up-to-date dependencies with security fixes and a coherent version story
  before the 1.0.0 restructure begins
