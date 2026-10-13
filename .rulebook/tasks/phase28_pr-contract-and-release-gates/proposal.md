# Proposal: phase28_pr-contract-and-release-gates

Source: docs/analysis/next-release-review-2026-10-12/ (finding E-06, priority P2, and the reproducible-release proposal in ecosystem.md)
Target release: 1.4.0

## Why

- **E-06**: pull requests only run the Rust workspace checks
  (`.github/workflows/rust-ci.yml`). Non-Rust SDK tests only run in the
  publish workflow (`.github/workflows/sdk-publish.yml`, on release or manual
  dispatch). Nothing checks the GUI, and the cross-language interop harness
  does not run on PRs. A broken server/client contract merges green and fails
  in consumers or at publish time, which is how E-03 and E-04 slipped through.
  Coverage produces LCOV but has no failing threshold, despite the documented
  95% policy.
- **Reproducibility**: `.gitignore` excludes `Cargo.lock`, so the dependency
  graph of a released binary cannot be reconstructed, and the project's own
  Rust rules say binaries commit their lockfile. `rust-toolchain.toml` and
  every workflow use a moving `nightly`, so two builds of the same commit can
  use different compilers.

## What Changes

- Commit `Cargo.lock` and build with `--locked` in CI and release.
- Pin `rust-toolchain.toml` to a dated nightly that every workflow reads.
- Pull requests run the TypeScript, Python, C# and PHP SDK tests, GUI
  type-check, lint and build, SDK contract tests against a built server, and
  the cross-language interop harness.
- Coverage is measured per component and enforced as a failing gate. Where a
  component is below the 95% policy, the measured value is the floor, the gap
  is recorded, and the gate fails on any drop.
- Package artifacts are tested against a built server before publishing.
- The gates are proven: a wrong command name and a changed amount mapping each
  fail a PR check.

## Impact

- Affected specs: ci (pull request and release gates)
- Affected code: `.github/workflows/`, `.gitignore`, `Cargo.lock`,
  `rust-toolchain.toml`, `scripts/interop/`, SDK and GUI package scripts
- Dependencies: phase26 and phase27 provide the contract cases the gates must
  catch
- Breaking change: NO
- User benefit: contract breaks are caught before merge, and every release
  binary can be rebuilt bit-for-bit in its dependency graph.
