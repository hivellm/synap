## ADDED Requirements

### Requirement: Bounded frame and payload allocation
Both binary protocols MUST reject a frame/bulk/array whose declared size exceeds a configured
maximum before allocating memory for it.

#### Scenario: Oversized RESP3 bulk is rejected
Given a client sends a bulk-string header declaring a length above the configured max
When the parser reads it
Then the server returns a protocol error and does not allocate the declared buffer

#### Scenario: Oversized SynapRPC frame is rejected
Given a client sends a 4-byte length prefix above the configured max frame size
When the codec reads the frame
Then the connection is dropped without allocating the declared buffer

### Requirement: Bounded pub/sub output buffers
Each pub/sub subscriber MUST have a bounded output buffer; a subscriber that cannot keep up is
dropped or disconnected rather than growing memory without limit.

#### Scenario: Slow consumer does not exhaust memory
Given a subscriber that stops reading on a high-rate topic
When published messages exceed the subscriber's buffer limit
Then the server drops or disconnects that subscriber and records a slow-consumer metric,
and other subscribers are unaffected

### Requirement: Connection limits
Each listener MUST honor a configurable maximum-connections limit and an idle-connection timeout.

#### Scenario: Connection flood is bounded
Given the max-connections limit is reached
When another client attempts to connect
Then the new connection is refused or queued rather than unconditionally accepted
