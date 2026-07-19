## ADDED Requirements

### Requirement: Idle-connection timeout
Each binary listener MUST close a connection that sends no data within a
configurable idle window.

#### Scenario: Idle connection is closed
Given a client connects and sends nothing
When the idle timeout elapses
Then the server closes the connection and frees its connection permit

### Requirement: Configurable network limits
The bulk/frame/aggregate/line caps, per-subscriber buffer size, max-connections,
and idle timeout MUST be configurable, defaulting to the phase6c values.

#### Scenario: Operator lowers the max-connections limit
Given a config that sets max-connections to a small value
When that many connections are open and another connects
Then the new connection is refused
