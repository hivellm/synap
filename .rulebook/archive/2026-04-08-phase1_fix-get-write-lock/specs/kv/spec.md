# KV Store Specification — GET Concurrency

## MODIFIED Requirements

### Requirement: Concurrent Read Scalability
The system MUST allow concurrent reads to the same shard without mutual exclusion,
except for writes that modify the data structure itself.

#### Scenario: Multiple readers do not block each other
Given 16 reader goroutines/tasks all GET distinct keys from the same shard simultaneously
When all GET operations are in flight concurrently
Then no reader MUST wait for another reader to release a lock
And throughput MUST scale with the number of readers up to the available CPU cores

#### Scenario: LRU timestamp updated without write lock
Given a key exists as `StoredValue::Expiring` with a `last_access: AtomicU32`
When GET is called on that key
Then `last_access` MUST be updated to the current time
And the update MUST be performed while holding only a read lock on the shard
And no write lock on the shard MUST be acquired during the GET operation

### Requirement: Read-Write Separation
The system MUST NOT use a write lock on shard data for operations that do not modify
the structure of the key-value store (i.e., do not insert, remove, or resize entries).

#### Scenario: GET is a read-only operation
Given any shard with any number of entries
When GET is called for a key in that shard
Then the shard data lock acquired MUST be a shared read lock
And concurrent SETs to the same shard MUST be allowed to proceed in parallel
