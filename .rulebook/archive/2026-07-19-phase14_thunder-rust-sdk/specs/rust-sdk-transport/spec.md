# Spec: Rust SDK RPC transport on Thunder

## MODIFIED Requirements

### Requirement: RPC transport implementation
The Rust SDK's `SynapRpc` transport SHALL be implemented on `thunder::Client`; the
SDK MUST NOT contain its own framing, request-id allocation or reconnect logic.

#### Scenario: Commands still round-trip
Given an SDK client in `TransportMode::SynapRpc` against a running server
When it issues `set` followed by `get`
Then the values returned match the pre-swap behavior

#### Scenario: Public API unchanged
Given code written against the previous SDK release
When it is compiled against this release
Then it compiles without source changes

### Requirement: Concurrent in-flight requests
The transport SHALL allow multiple requests to be in flight on one connection,
demultiplexed by frame id.

#### Scenario: Pipelined calls
Given one SDK client
When N commands are issued concurrently from N tasks
Then all N complete and each receives the response matching its own request

### Requirement: Timeouts and caps
The transport SHALL apply a connect timeout, a per-call timeout, and the Synap
frame cap on both encode and decode.

#### Scenario: Unreachable server
Given an address that accepts no connection
When the client connects
Then the call fails with a timeout error rather than hanging

#### Scenario: Oversized outbound frame
Given a command whose encoded body would exceed the configured frame cap
When it is sent
Then the client refuses it locally instead of writing it to the socket

## ADDED Requirements

### Requirement: Typed authentication errors
`NOAUTH` and `WRONGPASS` server errors SHALL surface as the SDK's authentication
error variant, not as an opaque string.

#### Scenario: Bad credentials
Given a server with `require_auth = true`
When the SDK connects with an invalid password
Then the returned error is the SDK's authentication variant

### Requirement: Registry-only dependencies
The Rust SDK SHALL declare no path dependencies.

#### Scenario: Dry-run publish
Given `sdks/rust/Cargo.toml`
When `cargo publish --dry-run` runs
Then it succeeds with every dependency resolved from a registry
