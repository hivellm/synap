# KV Store Specification — SET Path Allocation Efficiency

## MODIFIED Requirements

### Requirement: Value Clone Conditional on Cache
The system MUST NOT clone the value bytes during a SET operation when no cache layer is configured.

#### Scenario: No-cache path avoids clone
Given the server is started without an in-memory cache layer
When a SET request arrives with a 1 KiB value
Then the value bytes MUST be moved (not copied) into the shard storage
And no heap allocation for a value copy MUST occur on the critical path

### Requirement: Single Clock Read per SET
The system MUST read the system clock at most once per SET operation and reuse the result
for TTL calculation, WAL timestamping, and cache entry expiry.

#### Scenario: Clock read count
Given a SET request with a TTL
When the request is processed
Then `SystemTime::now()` or equivalent MUST be called exactly once
And the result MUST be passed to all consumers (storage, WAL, cache)

### Requirement: MSET Shard Batching
The system MUST process MSET operations by grouping keys per shard and acquiring each
shard lock exactly once for the full batch of keys targeting that shard.

#### Scenario: MSET acquires each shard lock once
Given an MSET of 64 keys spread across 8 shards (8 keys per shard)
When MSET is executed
Then each of the 8 shard write locks MUST be acquired exactly once
And all 8 keys belonging to that shard MUST be inserted under that single lock acquisition

#### Scenario: MSET throughput vs sequential SET
Given 64 key-value pairs targeting 8 distinct shards
When MSET is used instead of 64 sequential SETs
Then the MSET MUST complete in fewer total lock acquisitions
And observable throughput MUST be higher than sequential SET for the same batch
