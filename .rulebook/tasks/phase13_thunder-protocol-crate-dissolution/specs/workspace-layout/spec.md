# Spec: Workspace layout after protocol-crate dissolution

## REMOVED Requirements

### Requirement: `synap-protocol` workspace membership
`crates/synap-protocol` SHALL NOT be a workspace member after this task; no
first-party crate may depend on it.

#### Scenario: Workspace has no protocol crate
Given the root `Cargo.toml`
When the workspace members are listed
Then `crates/synap-protocol` is absent

#### Scenario: No in-repo consumer
Given every `Cargo.toml` under `crates/` and `sdks/`
When their dependencies are inspected
Then none names `synap-protocol`

## MODIFIED Requirements

### Requirement: RESP3 parser and HTTP envelope are server-internal
The RESP3 parser/writer and the HTTP envelope SHALL live inside `crates/synap-server`
and MUST NOT be published to any registry.

#### Scenario: RESP3 still parses
Given a RESP3 client connected to the RESP3 port
When it issues `SET k v` and `GET k`
Then the replies are byte-identical to the pre-move behavior

#### Scenario: Envelope still wraps HTTP responses
Given an HTTP request to a REST endpoint
When the handler returns
Then the response envelope is unchanged

## ADDED Requirements

### Requirement: Terminal deprecated shim
A final `synap-protocol` release SHALL consist solely of `#[deprecated]` re-exports
of `thunder::wire` under the historical type names, so external code compiling
against the old crate keeps compiling.

#### Scenario: External consumer still compiles
Given a crate that imports `synap_protocol::synap_rpc::types::SynapValue`
When it builds against the shim version
Then it compiles with a deprecation warning naming `thunder::wire`

### Requirement: SDK publishes without path dependencies
The Rust SDK SHALL publish with zero path dependencies and zero product-protocol
packages (Thunder amended Gate G2).

#### Scenario: Dry-run publish
Given the Rust SDK manifest
When `cargo publish --dry-run` runs
Then it succeeds and resolves every dependency from a registry
