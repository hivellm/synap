## ADDED Requirements

### Requirement: Dependency baseline before restructure
All 8 open dependabot PRs (#222-#229) MUST be merged or explicitly closed with rationale
before the `release/v1.0.0` branch is cut, and the three Cargo major bumps MUST each pass
`cargo check`, `cargo clippy -- -D warnings` and `cargo test` in isolation before merge.

#### Scenario: Cargo major bump gated individually
Given the rmcp 1.5.0→2.1.0 PR is open
When the PR branch is compiled and the full test suite runs
Then all MCP integration tests pass before the merge is performed

### Requirement: Single inherited workspace version
Every workspace member crate MUST inherit `version` and `edition` from
`[workspace.package]`, and the workspace version MUST read `1.0.0-rc.1`.

#### Scenario: No hardcoded member versions remain
Given the workspace manifests after this task
When `grep -L "version.workspace" */Cargo.toml crates/*/Cargo.toml sdks/rust/Cargo.toml` runs
Then no member manifest declares a literal version string
