# Tasks: Add Sorted Set Data Structure

## Core (25 commands, ~200 tasks, 6 weeks)

### Implementation
- [ ] Dual data structure (HashMap<Value,f64> + BTreeMap<OrderedFloat<f64>, HashSet<Value>>)
- [ ] 25+ commands (ZADD, ZREM, ZSCORE, ZRANK, ZRANGE, ZREVRANGE, ZRANGEBYSCORE, etc.)
- [ ] Weighted set operations (ZINTERSTORE, ZUNIONSTORE with WEIGHTS)
- [ ] TTL support, statistics, 20+ unit tests

### API
- [ ] 25 REST endpoints, 25 StreamableHTTP commands, 6 MCP tools

### Persistence
- [ ] 4 WAL operations (ZAdd, ZRem, ZIncrBy, ZRemRange)

### Testing
- [ ] 25+ unit tests, 20+ integration tests, 15+ benchmarks

### Performance Targets
- [ ] ZADD <200µs, ZSCORE <50µs, ZRANGE <1ms (100 items)

