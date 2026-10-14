# Snapshot Consistency and Expiry

## ADDED Requirements

### Requirement: Snapshot paired with its WAL position
Recovery from a snapshot plus the WAL MUST apply every acknowledged operation
exactly once.

#### Scenario: Write between cutoff and dump
Given a snapshot paused right after recording its WAL cutoff
When a client runs LPUSH `l` `x` and HINCRBY `h` `f` 1, and the snapshot then completes
And the server recovers from that snapshot and the WAL
Then `l` contains `x` once
And `h` field `f` was incremented once

### Requirement: Same-key write order
The WAL MUST record concurrent writes to the same key in the order they were
applied in memory.

#### Scenario: Concurrent APPEND
Given 8 clients appending distinct single bytes to key `k` concurrently 1000 times each
When the server recovers from the WAL
Then `k` equals the value it held before the restart

### Requirement: Absolute expiry survives restart
Snapshots, WAL records and replication MUST carry each key's absolute expiry,
and recovery MUST omit keys whose expiry has passed.

#### Scenario: Snapshot restart before expiry
Given key `k` with 100 seconds to live in a snapshot
When the server restarts 30 seconds later from that snapshot
Then `k` has at most 70 seconds to live

#### Scenario: Expired during downtime
Given key `k` with 10 seconds to live recorded only in the WAL
When the server restarts 60 seconds later
Then `k` does not exist

#### Scenario: Legacy record
Given a 1.3.2 WAL record setting `k` with a 100 second TTL at timestamp T
When 1.4.0 recovers it at T + 40 seconds
Then `k` has at most 60 seconds to live
