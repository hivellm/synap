## ADDED Requirements

### Requirement: Transactions are durable and replicated
Every command committed in an EXEC MUST be written to the WAL and propagated to
replicas as part of the commit.

#### Scenario: Committed transaction survives a crash
Given an EXEC that acknowledges success with sync durability enabled
When the process crashes immediately after
Then all of the transaction's writes are present after recovery

#### Scenario: Committed transaction reaches a replica
Given a connected master-replica pair
When the master commits an EXEC
Then the replica reflects all of the transaction's writes

### Requirement: Transactions isolated from concurrent writers
An EXEC MUST be isolated from non-transactional writers to the same keys.

#### Scenario: Concurrent plain write does not interleave
Given an EXEC in progress on key k
When another client issues a plain SET on k
Then the SET is ordered entirely before or after the EXEC, never between its commands
