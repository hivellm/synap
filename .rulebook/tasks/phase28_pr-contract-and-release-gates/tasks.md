## 1. Implementation
- [x] 1.1 Commit `Cargo.lock`, remove it from `.gitignore`, and build with `--locked` in CI and release — Cargo.lock tracked; every cargo build/check/test/clippy/bench/llvm-cov/deny/package/publish in rust-ci, rust-lint, release (`locked: true` on all 8 uploads), sdk-publish and the Dockerfile runs `--locked`; the Dockerfile also copies `crates/synap-cli/tests` (its `[[test]]` target broke the image build)
- [x] 1.2 Pin `rust-toolchain.toml` to a dated nightly and make every workflow use it — `nightly-2026-07-23` (rustc 1.99.0-nightly 6f72b5dd5 2026-07-22, the local toolchain); every workflow job and the Dockerfile install it through `.github/actions/setup-rust`, which reads the file; `cargo +nightly fmt` became `cargo fmt`
- [ ] 1.3 Run the TypeScript, Python, C# and PHP SDK tests on pull requests
- [ ] 1.4 Run GUI type-check, lint and build on pull requests
- [ ] 1.5 Run SDK contract tests against a built server and the cross-language interop harness on pull requests
- [ ] 1.6 Measure coverage per component, enforce it as a failing gate, and record the gap for any component below the 95% policy
- [ ] 1.7 Test package artifacts against a built server before publishing
- [ ] 1.8 Prove the gates: a wrong command name and a changed amount mapping each fail a PR check

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
