## MODIFIED Requirements

### Requirement: Memory accounting covers all datatypes
The `maxmemory` budget MUST account for Hash, List, Set, Sorted-Set, Stream and Queue memory,
not only the KV store.

#### Scenario: Collections trigger eviction
Given `maxmemory` set to a small limit and no KV data
When enough hash/list/set data is written to exceed the limit
Then the configured eviction/refusal policy fires, rather than the limit being silently exceeded

### Requirement: Reads avoid full value copies
Reading a value MUST NOT clone the entire payload; values are returned via a shared buffer.

#### Scenario: Large-value GET does not double memory
Given a multi-megabyte value
When it is read repeatedly
Then each read returns a shared buffer rather than allocating a full copy per read

## ADDED Requirements

### Requirement: Accurate memory reporting
INFO/metrics MUST report per-datatype memory usage that sums to the accounted total.

#### Scenario: Reported memory reflects reality
Given a mix of KV, hash and stream data
When memory metrics are scraped
Then per-datatype figures are present and their sum matches the accounted total used for eviction
