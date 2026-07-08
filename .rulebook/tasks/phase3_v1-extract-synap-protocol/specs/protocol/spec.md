## ADDED Requirements

### Requirement: Pure wire crate
The `synap-protocol` crate MUST contain only wire-format code (envelope, RESP3 parser,
RESP3 writer, RESP value type, SynapRPC codec, SynapRPC types) and MUST NOT depend on
`synap-server`, `synap-core`, or any store/handler/scripting code.

#### Scenario: No dependency cycle
Given the extracted `crates/synap-protocol/Cargo.toml`
When its `[dependencies]` section is inspected and `cargo check --workspace` runs
Then it lists no first-party crates and the workspace compiles

### Requirement: Dispatch stays server-side
The RESP3 command layer (`resp3/command/`) and SynapRPC dispatch layer
(`synap_rpc/dispatch/`) MUST remain inside `synap-server`.

#### Scenario: Wire behavior unchanged
Given the moved parser/writer/codec code with its unit tests
When the RESP3 parser/writer and SynapRPC codec test suites run in the new crate
Then all pre-existing tests pass without modification to assertions
