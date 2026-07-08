## ADDED Requirements

### Requirement: Replica applies every datatype
A replica MUST apply KV, Hash, List, Set, Sorted-Set, Queue and Stream operations
received from the master, converging to the master's state for all datatypes.

#### Scenario: Hash write converges on the replica
Given a connected master-replica pair
When the master executes an HSET
Then the replica reflects the same hash field after the operation is applied

### Requirement: Replication independent of persistence
Replication MUST propagate writes whether or not the WAL/persistence is enabled.

#### Scenario: Replication with persistence disabled
Given a master with persistence disabled and a connected replica
When the master processes a write
Then the write is still propagated to and applied by the replica
