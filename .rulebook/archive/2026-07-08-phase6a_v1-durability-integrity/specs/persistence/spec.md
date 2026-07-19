## MODIFIED Requirements

### Requirement: Snapshots persist every datatype
A snapshot MUST serialize and restore KV, Hash, List, Set, Sorted-Set, Queue and Stream data.
No datatype may depend solely on WAL replay for durability across a snapshot boundary.

#### Scenario: Hash survives a snapshot and restart
Given a server with hash fields written before a snapshot is taken
When the snapshot completes (advancing the WAL offset) and the process restarts
Then every hash field is present after recovery

#### Scenario: List, Set and Sorted-Set survive a snapshot and restart
Given list, set and sorted-set data written before a snapshot
When the process restarts from that snapshot
Then all three datatypes are fully restored

### Requirement: Snapshot integrity is verified on load
The loader MUST recompute the snapshot checksum and reject the snapshot on mismatch.

#### Scenario: Corrupt snapshot is rejected
Given a snapshot file whose bytes were altered after write
When the server attempts to load it
Then recovery returns a corruption error instead of loading partial/incorrect data

## ADDED Requirements

### Requirement: Single stream persistence path
Stream durability MUST use exactly one persistence path; WAL entries for streams are either
replayed on recovery or not written at all.

#### Scenario: No dead stream WAL entries
Given the persistence layer after this task
When a stream event is published and the server restarts
Then the stream state is recovered exactly once, with no WAL entry that recovery ignores
