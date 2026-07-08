## MODIFIED Requirements

### Requirement: MULTI/EXEC executes atomically
An EXEC block MUST execute as a single isolated unit; no other client's write may interleave
between two commands of the same transaction.

#### Scenario: Concurrent EXECs do not interleave
Given two clients each running a MULTI/EXEC that writes overlapping keys
When both EXEC concurrently
Then the observable result is equivalent to one transaction fully completing before the other,
never a mix of the two

### Requirement: WATCH provides correct optimistic concurrency
A watched key modified by another client between WATCH and EXEC MUST abort the transaction.

#### Scenario: WATCH aborts on concurrent modification
Given a client WATCHes a key and begins a transaction
When another client modifies that key before EXEC
Then EXEC aborts and applies none of its commands

### Requirement: Transactions are durable and replicated
Every command committed in an EXEC MUST be written to the WAL and to replication as part of
the commit.

#### Scenario: Committed transaction survives a crash
Given an EXEC that acknowledges success with sync durability enabled
When the process crashes immediately after
Then all of the transaction's writes are present after recovery
