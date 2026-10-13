# Atomic Snapshot Files

## ADDED Requirements

### Requirement: Snapshot files are published atomically
A snapshot MUST be written to a temporary file in the snapshot directory,
fsynced, and renamed to its final name before anything relies on it; the
directory MUST be fsynced after the rename on platforms that support it.
Recovery SHALL ignore temporary files.

#### Scenario: Crash while a snapshot is written
Given a server with a valid snapshot and acknowledged writes after it in the WAL
When the process dies after writing any number of bytes of the next snapshot
And the server restarts
Then the recovered state equals the acknowledged state
And the partial file is not loaded

#### Scenario: Leftover temporary file
Given a temporary snapshot file left by a crash
When the next snapshot is written
Then the temporary file is removed and the new snapshot is published

### Requirement: Unique, ordered snapshot names
Every snapshot MUST get a file name no other snapshot in the directory has,
and recovery SHALL order snapshots by the sequence in their names, not by the
clock. Files named by earlier releases SHALL load and count as older than any
sequenced file.

#### Scenario: Two snapshots in the same second
Given two snapshots written within the same second
When the snapshot directory is listed
Then both files exist under different names
And the newest one is the second snapshot

### Requirement: Safe fallback from a damaged newest snapshot
When the newest snapshot is truncated, fails its checksum or cannot be decoded,
recovery MUST fall back to the newest older valid snapshot only if the WAL
still holds every record after that snapshot's cutoff. Otherwise startup MUST
fail with an error naming the damaged files, and the server MUST NOT start
with empty stores.

#### Scenario: Truncated newest snapshot, WAL intact
Given snapshots S1 and S2 and acknowledged writes before, between and after them
When S2 is truncated at any byte position
And the server restarts
Then recovery loads S1, replays the WAL from S1's cutoff
And the recovered state equals the acknowledged state

#### Scenario: Damaged newest snapshot, WAL cut after it
Given a damaged newest snapshot and a WAL whose first record is after the previous snapshot's cutoff
When the server restarts
Then startup fails with an error that names the damaged snapshot
And no store is served empty

### Requirement: The WAL backs every published snapshot
Before a snapshot is published, the WAL MUST be fsynced up to the snapshot's
cutoff whatever the fsync mode, and after recovery the WAL MUST hand out
offsets at or above the cutoff of the snapshot that was loaded.

#### Scenario: WAL tail lost after a snapshot
Given a snapshot with cutoff C whose WAL lost its records below C
When the server restarts and acknowledges a new write
And the server restarts again
Then the new write is recovered

### Requirement: Retention after durability
Old snapshots MUST be removed only after the new snapshot is durable, and
retention MUST NOT remove the snapshot just written, even with
`max_snapshots: 0`.

#### Scenario: Retention keeps the only valid snapshot
Given `max_snapshots: 0`
When a snapshot is written
Then that snapshot still exists and loads
