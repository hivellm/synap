## 1. Dependabot upgrades

- [x] 1.1 Bump `zstd` to 0.14 in the workspace and verify the compressor still builds and round-trips
- [x] 1.2 Apply the ten TypeScript SDK bumps and regenerate both lockfiles
- [x] 1.3 Lint, type-check and build the TypeScript SDK on the new toolchain

## 2. Release 1.3.2

- [x] 2.1 Bump every publishable artifact to 1.3.2
- [x] 2.2 Cut the CHANGELOG `[1.3.2]` section and refresh the README badge

## 3. Verification

- [x] 3.1 Rust: fmt, clippy `-D warnings`, full workspace test suite
- [x] 3.2 TypeScript SDK: unit suite and S2S suite against a live server
- [x] 3.2a Fix the PFMERGE same-shard self-deadlock the S2S run surfaced, with a regression test
- [x] 3.2b Gate the integration tests behind a `full-tests` feature so plain `cargo test` stays fast
- [x] 3.3 Security audit: `cargo audit`, `cargo deny` and `npm`/`pnpm audit` clean
- [ ] 3.4 Merge `release/v1.3.2` into `main` and re-run the gate on the merged tree

## 4. Tail (docs + tests)

- [x] 4.1 Update or create documentation covering the implementation
- [x] 4.2 Write tests covering the new behavior
- [x] 4.3 Run tests and confirm they pass
