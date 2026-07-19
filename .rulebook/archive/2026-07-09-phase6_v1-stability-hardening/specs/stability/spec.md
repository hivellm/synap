## ADDED Requirements

### Requirement: No panics on user-reachable paths
Non-test code in `synap-core` and `synap-server` MUST NOT call `unwrap()`, `expect()` or
`panic!` on paths reachable from user input; each remaining call MUST carry a documented
invariant justification per the project Rust rule.

#### Scenario: Hostile input does not crash the server
Given a running server
When malformed RESP3 frames, oversized keys, and invalid UTF-8 payloads are sent
Then the server returns typed errors and stays alive

## MODIFIED Requirements

### Requirement: Consistent RESP3 enablement default
`Resp3Config::enabled` MUST default to the same value (true) whether the config is
deserialized from YAML with the field omitted or constructed via `Default`.

#### Scenario: YAML omits the resp3 block
Given a config.yml without `resp3.enabled`
When the server loads the config
Then the RESP3 listener starts, identical to a struct-built default config

### Requirement: Safe-by-default listener binding
Both the RESP3 and SynapRPC listeners MUST default to loopback (`127.0.0.1`); binding to
other interfaces MUST require an explicit host in the config.

#### Scenario: Fresh install exposure
Given a server started with an empty config
When listening sockets are enumerated
Then no Synap listener is bound to 0.0.0.0

### Requirement: Deterministic test synchronization
Integration tests MUST NOT rely on wall-clock sleeps for cross-task ordering; they use
notification primitives or bounded polling.

#### Scenario: Repeated suite runs are stable
Given the full test suite
When it runs 10 consecutive times in CI conditions
Then zero tests alternate between pass and fail
