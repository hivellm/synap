# WAL Lifecycle

## ADDED Requirements

### Requirement: Bounded loss window
In Periodic mode the WAL MUST flush and fsync buffered entries within the
configured interval whether or not more writes arrive. In Never mode it MUST
hand buffered entries to the OS after each group without calling fsync.

#### Scenario: Idle server
Given Periodic mode with a 1000 ms interval
When one entry is acknowledged and no further writes arrive for 2000 ms
Then the entry is on disk and is recovered after a process kill

#### Scenario: Never mode
Given Never mode
When one small entry is acknowledged
Then its bytes are in the WAL file without the buffer filling
And fsync is not called

### Requirement: Durable shutdown
Graceful shutdown MUST flush and fsync every acknowledged entry before the
process exits, and `PersistenceLayer::flush` MUST return only after buffered
entries reach the OS.

#### Scenario: Shutdown before the interval
Given Periodic mode with a 1000 ms interval
When an entry is acknowledged and the server shuts down gracefully 10 ms later
Then the entry is recovered on restart

### Requirement: Failures are reported
The WAL MUST fail every acknowledgement in a group whose flush or fsync fails,
and MUST fail every later append until restart. Every EXEC entry point MUST
return an error when its transaction could not be logged.

#### Scenario: Injected fsync failure
Given Always mode and a writer that fails the next fsync
When a client writes a key
Then the client receives an error
And the next write also receives an error
And the health endpoint reports the WAL failure

#### Scenario: EXEC with failed logging
Given a writer that fails the next flush
When a client runs EXEC over RESP3
Then EXEC returns an error stating the commit is not durable

### Requirement: Safe append point
On startup the WAL MUST verify frame checksums and lengths, MUST truncate a
torn tail to the last valid frame before appending, and MUST fail startup when
an invalid frame is followed by a valid one.

#### Scenario: Torn tail then new writes
Given a WAL whose last frame is cut inside its payload
When the server starts, acknowledges a new write, and restarts again
Then recovery contains every frame before the torn one and the new write

#### Scenario: Corrupt length
Given a WAL whose last frame header claims a length of 2^60 bytes
When the server starts
Then the frame is treated as a torn tail without attempting the allocation

#### Scenario: Interior corruption
Given a WAL with a checksum mismatch in a frame followed by valid frames
When the server starts
Then startup fails with an error giving the byte position of the bad frame
