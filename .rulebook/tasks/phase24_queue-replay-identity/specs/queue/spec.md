# Queue Recovery

## ADDED Requirements

### Requirement: Message identity survives recovery
Recovery and replication MUST restore each queue message with its original ID
and metadata.

#### Scenario: Unacknowledged message after restart
Given message `m` published to queue `q` and not consumed
When the server restarts from the WAL
Then `q` holds `m` with its original ID, priority and retry counters

#### Scenario: Replica parity
Given a master with one replica
When a client publishes three messages to `q`
Then the replica holds the same three message IDs as the master

### Requirement: Acknowledged messages stay acknowledged
Recovery MUST NOT make an acknowledged message deliverable again.

#### Scenario: WAL-only recovery after ACK
Given message `m` published, consumed and acknowledged
When the server restarts from the WAL alone
Then consuming from the queue does not return `m`

#### Scenario: Snapshot plus WAL recovery after ACK
Given message `m` captured in a snapshot and acknowledged after it
When the server restarts from the snapshot and the WAL
Then consuming from the queue does not return `m`

### Requirement: Replay errors are not hidden
Recovery MUST tolerate only a not-found result when replaying ACK or NACK, and
MUST fail on any other replay error.

#### Scenario: ACK of a message the snapshot already removed
Given a snapshot taken after `m` was acknowledged and a WAL that still holds the ACK
When the server recovers
Then recovery succeeds
