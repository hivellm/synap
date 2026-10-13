## ADDED Requirements

### Requirement: Dependabot queue absorbed into 1.3.2
The eleven open Dependabot PRs (#266, #269, #281, #283-#290) MUST be applied
together on `release/v1.3.2`, and the branch MUST pass `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, the full workspace test
suite, and the TypeScript SDK's lint, type-check, build, unit and S2S suites
before it is merged into `main`.

#### Scenario: zstd major bump keeps the compressor wire-compatible
Given `zstd` is resolved at 0.14
When the `synap-core` compression tests compress and decompress a payload
Then the decompressed bytes equal the input and the suite passes

#### Scenario: vitest 5 runs the existing suites unchanged
Given `vitest` and `@vitest/coverage-v8` are resolved at 5.0.1
When `npm run test:unit` and the S2S suite run against a live server
Then every test that passed under vitest 4 passes and none is skipped to get there

### Requirement: Single release version across artifacts
Every publishable artifact MUST read `1.3.2`: the Cargo workspace version, the
TypeScript, Python and C# SDK manifests, the Python `__version__`, and
`gui/package.json`.

#### Scenario: No artifact left on the previous version
Given the manifests after this task
When the version fields are grepped for `1.3.1`
Then only CHANGELOG history and archived task records still mention it
