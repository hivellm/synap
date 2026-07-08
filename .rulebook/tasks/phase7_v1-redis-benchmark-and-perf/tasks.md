## 1. Implementation
- [ ] 1.1 Annotate docs/analysis/synap-vs-redis/ findings with current status (RESOLVED/OPEN + evidence)
- [ ] 1.2 Verify RESP3 pipelining depth in resp3/server.rs (read-many-before-flush); fix if flushing per command
- [ ] 1.3 Add opt-in mimalloc allocator feature flag with bench comparison
- [ ] 1.4 Parallelize MGET/MSET by shard in kv_store with before/after bench numbers
- [ ] 1.5 Run redis-benchmark: Synap RESP3 vs Redis 7, -P 1 and -P 16, GET/SET/INCR/LPUSH/LRANGE/SADD
- [ ] 1.6 Measure native SynapRPC path with the existing bench harness
- [ ] 1.7 Publish results + methodology under docs/benchmarks/
- [ ] 1.8 Record ship/post-1.0 decision per remaining parity item (blocking ops, PSUBSCRIBE, keyspace notifications, SCAN cursors, LFU, IO threads); create a follow-up rulebook task for every post-1.0 item

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
