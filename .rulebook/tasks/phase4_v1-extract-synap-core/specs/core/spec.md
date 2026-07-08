## ADDED Requirements

### Requirement: Leaf engine crate
The `synap-core` crate MUST contain the data-structure engine (`core/`, `cache/`,
`compression/`, `simd/`) and MUST be a leaf in the first-party dependency graph:
no dependency on `synap-server` or `synap-protocol`.

#### Scenario: DAG holds
Given the extracted `crates/synap-core/Cargo.toml`
When its `[dependencies]` are inspected and `cargo check --workspace` runs
Then it lists no first-party crates and the workspace compiles

### Requirement: Engine behavior preserved
All engine unit tests MUST move with the code and pass unchanged; `SynapError` remains
the shared error type exported by `synap-core`.

#### Scenario: Server compiles against the new crates
Given `AppState` and the dispatch layers referencing `synap_core::*` and `synap_protocol::*`
When `cargo test` runs for the workspace
Then the full suite passes with the same test count as before the extraction
