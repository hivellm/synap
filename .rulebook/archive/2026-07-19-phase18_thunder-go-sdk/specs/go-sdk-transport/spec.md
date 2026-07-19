# Spec: Go SDK RPC transport and Thunder wire parity

## ADDED Requirements

### Requirement: `Bytes` decoding tolerance
The Go SDK's RPC transport SHALL decode `Value.Bytes` from both MessagePack `bin`
and the legacy array-of-integers form.

#### Scenario: Server-emitted bin
Given a Thunder-based Synap server that emits `Bytes` as MessagePack `bin`
When the Go SDK reads a binary value
Then it yields the identical byte sequence

#### Scenario: Legacy int-array
Given a frame carrying `Bytes` as an array of integers
When the Go SDK decodes it
Then it yields the identical byte sequence

### Requirement: Frame cap
The transport SHALL validate the length prefix against the Synap frame cap before
allocating a body buffer.

#### Scenario: Hostile length prefix
Given a peer that sends a length prefix above the cap
When the transport reads the header
Then it returns an error without allocating the claimed size

### Requirement: Upstream blocker is reported, never silently absorbed
If the Thunder Go module cannot be resolved from a registry, an issue SHALL exist on
`hivellm/thunder` describing the blocker, and a follow-up rulebook task SHALL exist
for the swap referencing it.

#### Scenario: Module unresolvable
Given `go list -m github.com/hivellm/thunder-go@latest` fails
When this task is archived
Then the upstream issue and the follow-up task both exist and are linked from the CHANGELOG

## MODIFIED Requirements

### Requirement: Public API stability
The Go SDK's exported client API SHALL be unchanged by this task.

#### Scenario: Existing consumer compiles
Given code written against the previous Go SDK release
When it is built against this release
Then it compiles without source changes
