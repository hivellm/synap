# KV Store Specification — Eviction Policies

## MODIFIED Requirements

### Requirement: Memory Pressure Eviction
The system MUST evict keys according to the configured `eviction_policy` when a SET operation
would exceed `max_memory_mb`, rather than immediately returning an error.

#### Scenario: allkeys-lru eviction under pressure
Given `eviction_policy` is `AllKeysLru` and memory is at 100% capacity
When a SET request arrives for a new key
Then the system MUST evict one or more least-recently-accessed keys to free space
And the SET MUST succeed without returning `MemoryLimitExceeded`

#### Scenario: noeviction returns error
Given `eviction_policy` is `NoEviction` and memory is at 100% capacity
When a SET request arrives for a new key
Then the system MUST return `MemoryLimitExceeded`
And no existing keys MUST be removed

#### Scenario: volatile-lru skips keys without TTL
Given `eviction_policy` is `VolatileLru`
And both TTL-bearing and persistent keys exist
When eviction is triggered
Then only keys with TTL MUST be eligible for eviction
And persistent (non-expiring) keys MUST NOT be removed

#### Scenario: volatile-ttl evicts soonest-expiring first
Given `eviction_policy` is `VolatileTtl`
And multiple keys have different TTLs remaining
When eviction is triggered
Then keys with the shortest remaining TTL MUST be evicted first

### Requirement: Steady-State Memory Bound
Given `max_memory_mb` is configured and an evicting policy is active,
the system MUST maintain `total_memory_bytes` at or below `max_memory_mb * 1024 * 1024`
indefinitely under continuous write load.

#### Scenario: Sustained write load stays bounded
Given `max_memory_mb=100` and `eviction_policy=AllKeysLru`
When writes of distinct 1 KiB values continue for 10 minutes
Then `total_memory_bytes` MUST remain within 105% of 100 MiB at all times
And no `MemoryLimitExceeded` errors MUST occur

## ADDED Requirements

### Requirement: Approximated LRU Sampling
The system MUST implement approximated LRU using random sampling per shard, consistent with
the Redis eviction algorithm design.

#### Scenario: Sample size is configurable
Given `eviction_sample_size` is set to N in `KVConfig`
When eviction is triggered on a shard
Then exactly N keys MUST be sampled randomly from that shard
And the oldest by `last_access` among the sample MUST be evicted
