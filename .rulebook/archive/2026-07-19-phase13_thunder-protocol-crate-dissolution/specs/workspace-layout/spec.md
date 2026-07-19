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

### Requirement: The crate is deleted, not shimmed
`crates/synap-protocol` SHALL be removed from the repository entirely. No
deprecation-shim release is published; the already-published 1.0.0 is
self-contained and remains available on crates.io for anyone pinned to it.

#### Scenario: The directory is gone
Given the repository at this release
When `crates/` is listed
Then `synap-protocol` is absent

#### Scenario: Migration guidance survives the deletion
Given an external consumer of `synap_protocol::synap_rpc::types::SynapValue`
When they consult the release notes
Then a type-by-type migration table to `thunder-rpc` is present in `CHANGELOG.md`

### Requirement: SDK publishes without path dependencies
The Rust SDK SHALL publish with zero path dependencies and zero product-protocol
packages (Thunder amended Gate G2).

#### Scenario: Dry-run publish
Given the Rust SDK manifest
When `cargo publish --dry-run` runs
Then it succeeds and resolves every dependency from a registry
