# KV Store Specification — SET Correctness

## MODIFIED Requirements

### Requirement: Memory Accounting Accuracy
The system MUST maintain `total_memory_bytes` within ±1% of actual allocated size across all
SET, overwrite, DELETE, and TTL expiration operations.

#### Scenario: Overwrite decrements old size
Given a key exists with value of size N bytes
When the same key is SET with a new value of size M bytes
Then `total_memory_bytes` MUST change by (M - N), not by M

#### Scenario: Delete decrements size
Given a key exists with value of size N bytes
When the key is DELeted
Then `total_memory_bytes` MUST decrease by N

#### Scenario: Expiration decrements size
Given a key with a TTL exists
When the TTL expires and `cleanup_expired()` removes it
Then `total_memory_bytes` MUST decrease by the entry's size

### Requirement: Stats Lock Elimination
The system MUST NOT acquire a global write lock during SET, GET, DEL, or EXPIRE operations
for the purpose of updating statistics.

#### Scenario: Concurrent SET throughput
Given 64 shards each accepting concurrent writes
When 16 threads simultaneously SET keys to different shards
Then no global write lock SHALL serialize the operations
And stats SHALL be updated via atomic `fetch_add` with `Relaxed` ordering

### Requirement: WAL Write-Ahead Semantics
The system MUST support a `durability: Sync` mode where WAL is flushed to disk before
responding to the client with success.

#### Scenario: Sync mode durability
Given `durability` is configured as `Sync`
When a SET request arrives
Then the WAL entry MUST be written and fsynced before the response is sent
And if WAL write fails, the key MUST NOT be written to memory

#### Scenario: Async mode behavior documented
Given `durability` is configured as `Async` (default)
When a SET request arrives
Then the response MAY be sent before the WAL is persisted
And the response MUST NOT claim durability that has not been guaranteed

### Requirement: Max Value Size Enforcement
The system MUST reject SET requests where the value size exceeds `max_value_size_bytes`
before performing any memory allocation.

#### Scenario: Oversized value rejected
Given `max_value_size_bytes` is configured to 1048576 (1 MiB)
When a SET request arrives with a 2 MiB value
Then the system MUST return an error without allocating memory for the value

### Requirement: INCR/DECR TTL Preservation
The system MUST preserve the existing TTL when INCR or DECR modifies a key that has an expiry.

#### Scenario: INCR preserves TTL
Given a key exists with value "42" and TTL of 30 seconds
When INCR is called on that key
Then the new value MUST be "43"
And the TTL MUST remain at approximately 30 seconds (not reset or destroyed)

#### Scenario: INCR overflow returns error
Given a key exists with value at i64::MAX
When INCR is called on that key
Then the system MUST return an overflow error
And the key value MUST NOT be modified
