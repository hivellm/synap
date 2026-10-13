# Data Migration

## ADDED Requirements

### Requirement: One persistence codec
`synap-migrate` MUST decode and encode snapshots and WAL files with the same
codec the server uses.

#### Scenario: Current server data directory
Given a data directory written by Synap 1.4.0 with a snapshot and a WAL tail
When `synap-migrate` runs on it
Then it finds the snapshot, applies the WAL tail, and reports every structure it read

#### Scenario: 1.3.2 data directory
Given a data directory written by Synap 1.3.2 with a v3 snapshot
When `synap-migrate` runs on it
Then the migration succeeds

### Requirement: Faithful migration into Hub mode
The migrated output MUST boot a Hub-mode server whose tenant-scoped data
matches the source, including queue and stream metadata and remaining expiry.

#### Scenario: Round trip through the real server
Given a fixture holding KV, hash, list, set, sorted set, queue and stream data, some keys with expiry
When it is migrated for tenant `t` and a Hub-mode server boots on the output
Then tenant `t` reads every value, message ID and stream offset of the source
And no key's remaining time to live is longer than in the source

### Requirement: Source data is never modified
`synap-migrate` MUST validate the whole input before writing, MUST write only
to a new location, and MUST leave the source directory unchanged.

#### Scenario: Corrupt input
Given a data directory whose snapshot checksum does not match
When `synap-migrate` runs on it
Then it exits with an error naming the file
And no output is written
And the source directory is byte-for-byte unchanged
