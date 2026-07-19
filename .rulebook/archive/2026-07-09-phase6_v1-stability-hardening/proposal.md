# Proposal: phase6_v1-stability-hardening

Source: docs/analysis/synap-v1-release/ (F-011, F-012, F-013, F-014, F-015)

## Why
A 1.0 label promises stability, and the analysis found four concrete gaps. (1) Tests:
156 `sleep(`/`#[ignore]`/flaky markers across 1,441 test fns, with a documented history of
deflaking commits — timing-based synchronization erodes CI confidence exactly when the
restructure needs it most. (2) Hot paths: `core/` alone has 442 `unwrap()` hits across
65 non-test files containing `unwrap/expect/panic!`, violating the project Rust rule; a
reachable `unwrap` on user input is a remote-DoS vector for a datastore. (3) Config:
`Resp3Config::enabled` deserializes to `false` when omitted from YAML but `impl Default`
says `true` — the flagship Redis-compat listener silently changes behavior depending on how
the config was built. (4) Exposure: RESP3 binds `127.0.0.1` by default while SynapRPC binds
`0.0.0.0` — two binary protocols with opposite security postures. Plus 13 TODO/FIXME markers
that a 1.0 should not ship.

## What Changes
1. Flaky-test audit: run the suite repeatedly (≥10 iterations) to identify real flakes;
   replace `sleep`-based synchronization with `tokio::sync::Notify`/barriers/
   polling-with-deadline; re-enable or justify each `#[ignore]`.
2. `unwrap()/expect()/panic!` triage in non-test code (core first, then server): each hit is
   either (a) converted to `?` + typed `SynapError`, (b) proven unreachable and documented
   per the rust-rule invariant exception, or (c) test-only. Zero unaudited hits remain.
3. Fix `Resp3Config`: `#[serde(default = "default_true")]` (or equivalent) so YAML-omitted
   and struct-built configs agree; regression test for both load paths.
4. Unify listener bind defaults: both RESP3 and SynapRPC default to loopback; exposing on
   `0.0.0.0` becomes an explicit config choice, documented in config.example.yml.
5. Burn down the 13 TODO/FIXME markers: implement, or convert to tracked GitHub issues and
   remove the marker (no-shortcuts rule: no orphan TODOs at 1.0).

## Impact
- Affected specs: config behavior spec for RESP3/SynapRPC defaults (MODIFIED)
- Affected code: `crates/synap-server/src/config.rs`, `crates/synap-core/src/**` (unwrap triage),
  `crates/synap-server/tests/**` (deflaking), scattered TODO sites
- Breaking change: YES (soft) — SynapRPC default bind moves 0.0.0.0 → 127.0.0.1; called out
  in CHANGELOG with one-line migration (set host explicitly)
- User benefit: no panics on hostile input, deterministic CI, predictable config behavior,
  safe-by-default network exposure
