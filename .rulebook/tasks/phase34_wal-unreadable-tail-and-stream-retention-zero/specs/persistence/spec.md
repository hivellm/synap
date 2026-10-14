# Persistence — WAL records this release cannot read

## ADDED Requirements

### Requirement: Unreadable WAL records refuse startup
When opening the WAL finds that it ends in a complete frame whose checksum
matches but whose record does not decode, the server MUST refuse to start and
MUST leave the WAL file unchanged. The error SHALL name the WAL path, the byte
offset of the record and that a newer Synap release may have written it.

#### Scenario: Record of an unknown type at the end of the WAL
Given a WAL whose last frame is complete, matches its checksum and holds a record type this release does not know
When the server starts on that data directory
Then recovery and the WAL open fail with an error that refuses startup
And the error names the WAL path, the byte offset of that frame and a newer release
And the WAL file is byte-for-byte unchanged

### Requirement: Crash leftovers are still repaired
The server SHALL truncate a WAL tail that a crash can leave (a frame cut short,
a frame failing its checksum, or zero-filled bytes) to the end of the last
valid frame, and MUST then start and accept writes.

#### Scenario: Every crash shape
Given a WAL whose last bytes are a cut-short frame, a frame failing its checksum, or zero fill
When the server starts on it and logs a new write
Then the tail is truncated to the last valid frame
And the next start recovers every record before the tail plus the new write
