## ADDED Requirements

### Requirement: Replication is active in the running server
When `config.replication.enabled` is true, the server MUST instantiate and run the
master or replica node and feed the live write path into replication.

#### Scenario: Master accepts and replicates writes
Given a server started with role=master and a replica listen address
When a write occurs and a replica is connected
Then the replica receives and applies that write

#### Scenario: Replica connects and stays in sync
Given a server started with role=replica pointing at a master
When the master processes writes
Then the replica's state converges to the master's for every datatype

### Requirement: All datatypes are replicated
Replication MUST cover KV, Hash, List, Set, Sorted-Set, Queue and Stream operations, not KV only.

#### Scenario: Hash write reaches the replica
Given a connected master-replica pair
When the master executes an HSET
Then the replica reflects the same hash field

### Requirement: Resync correctness
A replica reconnecting after a disconnect MUST use partial resync when its offset is still in
the master's backlog, and full resync otherwise.

#### Scenario: Partial resync after brief disconnect
Given a replica that briefly disconnects and reconnects within the backlog window
When it resyncs
Then it receives only the missed operations, not a full snapshot
