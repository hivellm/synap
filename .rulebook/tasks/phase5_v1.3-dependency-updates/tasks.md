## 1. Implementation
- [ ] 1.1 Adopt `@hivehub/thunder` 0.2.2 in `sdks/typescript` (PR #250) and refresh the lockfile
- [ ] 1.2 Check crates.io/NuGet for `thunder-rpc`/`HiveLLM.Thunder` 0.2.2; if present align workspace `Cargo.toml`, `sdks/rust`, `sdks/csharp` to it, else realign `sdks/rust/Cargo.toml` thunder-rpc floor to the workspace version
- [ ] 1.3 Adopt the four TS dev-tooling bumps: eslint-plugin (#251), typescript-eslint (#248), parser (#247), prettier (#249)
- [ ] 1.4 Run the TS SDK lint + format check; fix (do not suppress) any new findings the tooling bumps surface

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation (CHANGELOG [Unreleased] "Changed" — dependency bumps; note aligned Thunder version)
- [ ] 2.2 Write tests covering the new behavior (no new tests — rely on existing SDK interop/wire tests to prove the Thunder patch stays protocol-compatible)
- [ ] 2.3 Run tests and confirm they pass (TS SDK build + tests; `cargo check` + `clippy -D warnings` + suite if any Cargo manifest changed; server↔SDK interop for Thunder)
