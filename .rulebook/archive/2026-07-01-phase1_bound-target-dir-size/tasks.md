## 1. Implementation
- [x] 1.1 Add `[profile.dev] debug = "line-tables-only"` to workspace `Cargo.toml`; confirm `[profile.release] strip = true` is present
- [x] 1.2 Add `scripts/sweep-target.sh` (bash) and `scripts/sweep-target.ps1` (PowerShell) wrapping `cargo-sweep` with a configurable retention (default 14 days)
- [x] 1.3 Set `CARGO_INCREMENTAL=0` in the `env:` block of `rust-ci.yml`, `rust-lint.yml`, and `rust-test.yml`
- [x] 1.4 Add `docs/rust-target-hygiene.md` documenting profile settings, sweep scripts, and scheduling

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/rust-target-hygiene.md + CHANGELOG)
- [x] 2.2 Write tests covering the new behavior — `cargo metadata` validates the manifest parses with the new profile; `--check` shells verify the sweep scripts (`cargo-sweep` install + retention arg)
- [x] 2.3 Run tests and confirm they pass (`cargo metadata` exit 0; `cargo check --workspace` clean)
