# Proposal: phase1_bound-target-dir-size

Source: GitHub issue #211 — "Bound `target/` directory size: line-tables-only
debuginfo + cargo-sweep"

## Why
Cargo never garbage-collects `target/`: stale object files, incremental caches,
and rlibs from old dependency versions accumulate indefinitely. Combined with the
dev profile's default *full debuginfo* for every crate and dependency, `target/`
grows without bound — the sibling `hivellm/cortex` repo hit 500+ GB before anyone
noticed. This Rust + Cargo repo has the same exposure and no mitigation today.

## What Changes
1. **`Cargo.toml` profile settings** (the biggest size lever):
   - `[profile.dev] debug = "line-tables-only"` — keeps `file:line` in panics and
     backtraces while slashing debuginfo size and speeding incremental rebuilds.
   - `[profile.release] strip = true` — already present in this repo (verify/keep).
2. **Cleanup scripts** `scripts/sweep-target.sh` + `scripts/sweep-target.ps1`
   wrapping [`cargo-sweep`](https://github.com/holmgr/cargo-sweep): remove
   artifacts not accessed in N days (default 14) without breaking incrementality.
3. **CI**: set `CARGO_INCREMENTAL=0` in the Rust workflows — CI starts from a cold
   cache, so incremental compilation only adds artifacts and slows the build.
4. **Docs**: `docs/rust-target-hygiene.md` explaining the settings, the sweep
   scripts, and how to schedule the sweep (cron / Task Scheduler).

## Impact
- Affected specs: none (build/infra hygiene, no runtime behavior change)
- Affected code: `Cargo.toml`, `.github/workflows/rust-*.yml`,
  `scripts/sweep-target.{sh,ps1}`, `docs/rust-target-hygiene.md`
- Breaking change: NO
- User benefit: bounded disk usage; faster incremental dev rebuilds; still
  retains `file:line` in panics/backtraces for debugging.
