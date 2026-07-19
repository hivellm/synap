## ADDED Requirements

### Requirement: Published Redis comparison
The project MUST publish reproducible benchmark results comparing Synap (RESP3 listener)
against Redis 7 on the same host, covering GET, SET, INCR, LPUSH, LRANGE and SADD at
pipeline depths 1 and 16, with methodology documented under `docs/benchmarks/`.

#### Scenario: Reproducible run
Given the documented benchmark methodology
When a developer follows it on a clean machine with Redis 7 and Synap installed
Then redis-benchmark produces comparable result tables for both servers

### Requirement: Multi-key operations parallelized by shard
`MGET`/`MSET` MUST resolve keys grouped by shard rather than sequentially key-by-key,
with a before/after benchmark demonstrating the improvement.

#### Scenario: MGET across shards
Given 1,000 keys distributed across the 64 shards
When MGET requests all of them
Then per-shard lookups execute grouped, and the recorded benchmark shows lower latency
than the sequential baseline

### Requirement: Explicit parity decisions
Every remaining Redis-parity gap (blocking ops, PSUBSCRIBE, keyspace notifications,
SCAN cursors, LFU eviction, IO threads) MUST have a recorded ship/post-1.0 decision, and
every post-1.0 item MUST have a follow-up rulebook task created before this task archives.

#### Scenario: No orphan gaps
Given the analysis README after this task
When the parity table is reviewed
Then each item shows either "shipped in 1.0" with evidence or a follow-up task ID
