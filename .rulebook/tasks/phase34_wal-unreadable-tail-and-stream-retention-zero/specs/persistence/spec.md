# Persistence — WAL records this release cannot read

## ADDED Requirements

### Requirement: Unreadable WAL records refuse startup
When opening the WAL finds that its first invalid frame is complete, matches
its checksum and holds a record that does not decode, whether it is the last
frame or more frames follow it, the server MUST refuse to start and MUST leave
the WAL file unchanged. The error SHALL name the WAL path, the byte offset of
the record and that a newer Synap release may have written it.

#### Scenario: Record of an unknown type at the end of the WAL
Given a WAL whose last frame is complete, matches its checksum and holds a record type this release does not know
When the server starts on that data directory
Then recovery and the WAL open fail with an error that refuses startup
And the error names the WAL path, the byte offset of that frame and a newer release
And the WAL file is byte-for-byte unchanged

#### Scenario: Record of an unknown type followed by more frames
Given a WAL in which a complete frame holding a record type this release does not know is followed by a valid frame, by another such record, or by a frame cut short
When the WAL is opened
Then the open fails with the same error, naming the byte offset of the unknown record
And the WAL file is byte-for-byte unchanged

### Requirement: Damage before an intact record is interior corruption
A frame that is cut short, fails its checksum or is zero fill, followed by a
complete frame whose checksum matches, MUST make startup refuse as interior
corruption, naming the byte offsets of both frames, whether or not this release
can decode the record that follows; the server MUST NOT truncate that record
as part of a torn tail, and the migrator MUST NOT read it as one.

#### Scenario: Damaged frame before a record of an unknown type
Given a WAL in which a frame failing its checksum, or zero fill, is followed by a complete frame whose checksum matches and whose record type this release does not know
When the server starts on that data directory
Then recovery and the WAL open fail with an interior corruption error that refuses startup
And the error names the byte offset of the damaged frame and of the record after it
And the WAL file is byte-for-byte unchanged
And `synap-migrate` fails its read of that WAL instead of reporting a torn tail

### Requirement: Crash leftovers are still repaired
The server SHALL truncate a WAL tail that a crash can leave (a frame cut short,
a frame failing its checksum, or zero-filled bytes) to the end of the last
valid frame when no complete frame whose checksum matches follows it, and MUST
then start and accept writes.

#### Scenario: Every crash shape
Given a WAL whose last bytes are a cut-short frame, a frame failing its checksum, or zero fill
When the server starts on it and logs a new write
Then the tail is truncated to the last valid frame
And the next start recovers every record before the tail plus the new write
