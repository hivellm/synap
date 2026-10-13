# Atomic Transaction Records

## ADDED Requirements

### Requirement: Crash-atomic transactions
The WAL MUST store each committed transaction as a single record, and recovery
MUST apply either every operation of the transaction or none of them.

#### Scenario: Truncation at every byte
Given a WAL whose last record is a three-operation transaction
When the file is truncated at each byte position inside that record and the server recovers
Then each recovery contains either all three effects or none of them

#### Scenario: Transaction larger than the buffer
Given a transaction whose encoded size exceeds the writer buffer
When it commits and the server crashes after the acknowledgement
Then recovery contains every effect of the transaction

### Requirement: Versioned WAL format
The WAL MUST record its format version, MUST read headerless files as the
legacy format, and MUST refuse to start on an unknown newer version.

#### Scenario: Legacy WAL
Given a WAL written by Synap 1.3.2
When Synap 1.4.0 starts on it
Then every entry is replayed

#### Scenario: Unknown version
Given a WAL whose header declares a version newer than the server supports
When the server starts
Then startup fails with an error naming both versions

### Requirement: Atomic replication of transactions
The master MUST send a transaction to replicas as one unit, and a replica MUST
apply it whole or not at all.

#### Scenario: Replica disconnect mid-transaction
Given a replica receiving a multi-operation transaction
When the connection drops before the unit is complete
Then the replica holds none of the transaction's effects
And it receives the whole transaction after resync
