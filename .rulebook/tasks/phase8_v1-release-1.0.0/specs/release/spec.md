## ADDED Requirements

### Requirement: Coherent 1.0.0 release
The release MUST ship with `[workspace.package] version = "1.0.0"`, a CHANGELOG entry
containing a migration guide for the crate split (old `synap_server::core::*` /
`synap_server::protocol::*` paths → `synap_core::*` / `synap_protocol::*`) and the
SynapRPC bind-default change, and all user-facing docs describing the new `crates/` layout.

#### Scenario: Docs match reality
Given the tagged v1.0.0 tree
When README/AGENTS.override.md workspace trees are compared to the actual directory layout
Then they are identical

### Requirement: Full release gate
The `release/v1.0.0` branch MUST pass, in order: `cargo check --workspace`,
`cargo clippy -- -D warnings`, `cargo fmt --check`, the full test suite, bench
compilation, and a Docker image build, before the `v1.0.0` tag is created.

#### Scenario: Gate is green
Given the release candidate commit
When the six gate steps execute sequentially
Then every step exits with code 0 and the tag + release PR are created afterwards
