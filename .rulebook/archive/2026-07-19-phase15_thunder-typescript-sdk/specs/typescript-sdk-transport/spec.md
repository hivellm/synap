# Spec: TypeScript SDK RPC transport on Thunder

## MODIFIED Requirements

### Requirement: RPC transport implementation
The TypeScript SDK's SynapRPC transport SHALL be implemented on `@hivehub/thunder`;
the SDK MUST NOT contain its own framing, msgpack codec or reconnect logic.

#### Scenario: Commands still round-trip
Given a client in RPC mode against a running server
When it issues `set` followed by `get`
Then the values returned match the pre-swap behavior

#### Scenario: Public API unchanged
Given application code written against the previous SDK release
When it runs against this release
Then no source change is required

### Requirement: Frame cap
The transport SHALL validate the length prefix against the configured cap before
allocating a body buffer.

#### Scenario: Hostile length prefix
Given a peer that sends a length prefix above the cap
When the transport reads the header
Then it rejects the frame and closes the connection without allocating the claimed size

## ADDED Requirements

### Requirement: `Bytes` decoding tolerance
The transport SHALL decode `Value.Bytes` from both MessagePack `bin` and the legacy
array-of-integers form.

#### Scenario: Server-emitted bin
Given a server that emits `Bytes` as MessagePack `bin`
When the SDK reads a binary value
Then it yields the identical byte sequence

#### Scenario: Legacy int-array
Given a frame carrying `Bytes` as an array of integers
When the SDK decodes it
Then it yields the identical byte sequence

### Requirement: Push delivery
Pub/sub messages SHALL be delivered through Thunder's push hook on the same
connection, not a second socket.

#### Scenario: Published message reaches a subscriber
Given a subscribed client
When another client publishes to the topic
Then the subscriber's handler receives the message
