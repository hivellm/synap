## 1. Implementation
- [ ] 1.1 Commit `Cargo.lock`, remove it from `.gitignore`, and build with `--locked` in CI and release
- [ ] 1.2 Pin `rust-toolchain.toml` to a dated nightly and make every workflow use it
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
