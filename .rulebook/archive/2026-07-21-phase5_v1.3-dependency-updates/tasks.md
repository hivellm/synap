## 1. Implementation
- [x] 1.1 Adopt `@hivehub/thunder` 0.2.2 in `sdks/typescript` (PR #250) and refresh the lockfile — lockfile regenerated from manifests (stale node_modules anchored npm's resolver; regenerated clean, resolves 0.2.2)
- [x] 1.2 Check crates.io/NuGet for `thunder-rpc`/`HiveLLM.Thunder` 0.2.2; if present align workspace `Cargo.toml`, `sdks/rust`, `sdks/csharp` to it, else realign `sdks/rust/Cargo.toml` thunder-rpc floor to the workspace version — 0.2.2 present on all three registries; workspace, sdks/rust and csproj all aligned to 0.2.2
- [x] 1.3 Adopt the four TS dev-tooling bumps: eslint-plugin (#251), typescript-eslint (#248), parser (#247), prettier (#249) — trio at 8.65.0, prettier 3.9.6
- [x] 1.4 Run the TS SDK lint + format check; fix (do not suppress) any new findings the tooling bumps surface — ran both; zero NEW findings from the bumps (prettier 3.9.5 baseline reports the same 51 files; eslint core unchanged at 10.7.0, all 21 errors/128 warnings pre-existing debt, out of scope per surgical-changes rule)

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation (CHANGELOG [Unreleased] "Changed" — dependency bumps; note aligned Thunder version)
- [x] 2.2 Write tests covering the new behavior (no new tests — rely on existing SDK interop/wire tests to prove the Thunder patch stays protocol-compatible)
- [x] 2.3 Run tests and confirm they pass — TS: build + 379 unit tests green; Rust: `cargo check` + `clippy -D warnings` clean, 90 suites green incl. `synap_rpc_thunder_tests`/`sdk_rpc_e2e_tests` (wire interop); C#: build 0 warnings, 107 tests green
