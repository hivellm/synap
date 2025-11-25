# Tasks: Add Sorted Set Data Structure

## Phase 1: Core Implementation (Week 1-2)

### Core Data Structure
- [ ] Create `SortedSetStore` struct with 64-way sharding
- [ ] Implement `SortedSetValue` with dual storage:
  - [ ] `HashMap<Vec<u8>, OrderedFloat<f64>>` - member to score lookup
  - [ ] `BTreeMap<(OrderedFloat<f64>, Vec<u8>), ()>` - sorted index
- [ ] Implement `OrderedFloat` wrapper for f64 with total ordering
- [ ] Add TTL support for entire sorted set
- [ ] Add statistics tracking (`SortedSetStats`)
- [ ] Write 10+ unit tests for core structure

### Basic Commands (Week 1)
- [x] Add `ZADD` operation (add/update members with scores) - TypeScript SDK
  - [ ] Support NX (only add new) option
  - [ ] Support XX (only update existing) option
  - [ ] Support GT (only update if new score > old) option
  - [ ] Support LT (only update if new score < old) option
  - [ ] Support CH (return count of changed elements)
  - [ ] Support INCR (increment score) option
- [x] Add `ZREM` operation (remove members) - TypeScript SDK
- [x] Add `ZSCORE` operation (get member score) - TypeScript SDK
- [x] Add `ZCARD` operation (get cardinality/count) - TypeScript SDK
- [x] Add `ZINCRBY` operation (increment member score) - TypeScript SDK
- [ ] Add `ZMSCORE` operation (get multiple scores)
- [x] Write 15+ tests for basic operations - TypeScript SDK tests added

## Phase 2: Range & Ranking Commands (Week 2-3)

### Range Operations
- [x] Add `ZRANGE` operation (get range by index) - TypeScript SDK
  - [x] Support WITHSCORES option - TypeScript SDK
  - [ ] Support REV (reverse) option
  - [ ] Support BYSCORE option
  - [ ] Support BYLEX option
  - [ ] Support LIMIT offset count
- [ ] Add `ZREVRANGE` operation (reverse range by index)
- [x] Add `ZRANGEBYSCORE` operation (range by score) - TypeScript SDK
  - [ ] Support min/max with inclusive/exclusive bounds
  - [x] Support WITHSCORES option - TypeScript SDK
  - [ ] Support LIMIT offset count
- [ ] Add `ZREVRANGEBYSCORE` operation (reverse range by score)
- [ ] Add `ZRANGEBYLEX` operation (range by lexicographical order)
- [ ] Add `ZREVRANGEBYLEX` operation (reverse range by lex)
- [x] Write 20+ tests for range operations - TypeScript SDK tests added

### Ranking Commands
- [x] Add `ZRANK` operation (get member rank, 0-based) - TypeScript SDK
- [x] Add `ZREVRANK` operation (get reverse rank) - TypeScript SDK
- [x] Add `ZCOUNT` operation (count members in score range) - TypeScript SDK
- [ ] Add `ZLEXCOUNT` operation (count in lex range)
- [x] Write 10+ tests for ranking operations - TypeScript SDK tests added

## Phase 3: Advanced Operations (Week 3-4)

### Pop & Remove Operations
- [x] Add `ZPOPMIN` operation (remove and return lowest scored) - TypeScript SDK
  - [x] Support count parameter - TypeScript SDK
- [x] Add `ZPOPMAX` operation (remove and return highest scored) - TypeScript SDK
  - [x] Support count parameter - TypeScript SDK
- [ ] Add `BZPOPMIN` operation (blocking pop min)
- [ ] Add `BZPOPMAX` operation (blocking pop max)
- [ ] Add `ZREMRANGEBYRANK` operation (remove by rank range)
- [ ] Add `ZREMRANGEBYSCORE` operation (remove by score range)
- [ ] Add `ZREMRANGEBYLEX` operation (remove by lex range)
- [x] Write 15+ tests for pop/remove operations - TypeScript SDK tests added

### Set Operations (Complex)
- [ ] Add `ZINTERSTORE` operation (intersection with weights)
  - [ ] Support multiple source sets
  - [ ] Support WEIGHTS parameter
  - [ ] Support AGGREGATE (SUM, MIN, MAX)
- [ ] Add `ZUNIONSTORE` operation (union with weights)
  - [ ] Support multiple source sets
  - [ ] Support WEIGHTS parameter
  - [ ] Support AGGREGATE (SUM, MIN, MAX)
- [ ] Add `ZDIFFSTORE` operation (difference)
- [ ] Add `ZINTER` operation (intersection without store)
- [ ] Add `ZUNION` operation (union without store)
- [ ] Add `ZDIFF` operation (difference without store)
- [ ] Implement weighted score aggregation
- [ ] Write 20+ tests for set operations

### Scan Operation
- [ ] Add `ZSCAN` operation (cursor-based iteration)
  - [ ] Support MATCH pattern
  - [ ] Support COUNT hint
- [ ] Implement cursor state management
- [ ] Write 5+ tests for scan

## Phase 4: API Exposure (Week 4)

### REST API (25 endpoints)
- [ ] POST `/sortedset/{key}/zadd` - Add members
- [ ] POST `/sortedset/{key}/zrem` - Remove members
- [ ] GET `/sortedset/{key}/zscore` - Get score
- [ ] GET `/sortedset/{key}/zcard` - Get cardinality
- [ ] POST `/sortedset/{key}/zincrby` - Increment score
- [ ] GET `/sortedset/{key}/zmscore` - Get multiple scores
- [ ] GET `/sortedset/{key}/zrange` - Range by index
- [ ] GET `/sortedset/{key}/zrevrange` - Reverse range
- [ ] GET `/sortedset/{key}/zrangebyscore` - Range by score
- [ ] GET `/sortedset/{key}/zrevrangebyscore` - Reverse range by score
- [ ] GET `/sortedset/{key}/zrangebylex` - Range by lex
- [ ] GET `/sortedset/{key}/zrevrangebylex` - Reverse range by lex
- [ ] GET `/sortedset/{key}/zrank` - Get rank
- [ ] GET `/sortedset/{key}/zrevrank` - Get reverse rank
- [ ] GET `/sortedset/{key}/zcount` - Count in score range
- [ ] GET `/sortedset/{key}/zlexcount` - Count in lex range
- [ ] POST `/sortedset/{key}/zpopmin` - Pop minimum
- [ ] POST `/sortedset/{key}/zpopmax` - Pop maximum
- [ ] POST `/sortedset/{key}/zremrangebyrank` - Remove by rank
- [ ] POST `/sortedset/{key}/zremrangebyscore` - Remove by score
- [ ] POST `/sortedset/{key}/zremrangebylex` - Remove by lex
- [ ] POST `/sortedset/zinterstore` - Intersection store
- [ ] POST `/sortedset/zunionstore` - Union store
- [ ] POST `/sortedset/zdiffstore` - Difference store
- [ ] GET `/sortedset/stats` - Statistics

### StreamableHTTP (25 commands)
- [ ] Add `sortedset.zadd` command
- [ ] Add `sortedset.zrem` command
- [ ] Add `sortedset.zscore` command
- [ ] Add `sortedset.zcard` command
- [ ] Add `sortedset.zincrby` command
- [ ] Add `sortedset.zmscore` command
- [ ] Add `sortedset.zrange` command
- [ ] Add `sortedset.zrevrange` command
- [ ] Add `sortedset.zrangebyscore` command
- [ ] Add `sortedset.zrevrangebyscore` command
- [ ] Add `sortedset.zrangebylex` command
- [ ] Add `sortedset.zrevrangebylex` command
- [ ] Add `sortedset.zrank` command
- [ ] Add `sortedset.zrevrank` command
- [ ] Add `sortedset.zcount` command
- [ ] Add `sortedset.zlexcount` command
- [ ] Add `sortedset.zpopmin` command
- [ ] Add `sortedset.zpopmax` command
- [ ] Add `sortedset.zremrangebyrank` command
- [ ] Add `sortedset.zremrangebyscore` command
- [ ] Add `sortedset.zremrangebylex` command
- [ ] Add `sortedset.zinterstore` command
- [ ] Add `sortedset.zunionstore` command
- [ ] Add `sortedset.zdiffstore` command
- [ ] Add `sortedset.stats` command

### MCP Tools (6 tools)
- [ ] Create MCP tool: `synap_sortedset_zadd`
- [ ] Create MCP tool: `synap_sortedset_zrem`
- [ ] Create MCP tool: `synap_sortedset_zrange`
- [ ] Create MCP tool: `synap_sortedset_zrank`
- [ ] Create MCP tool: `synap_sortedset_zscore`
- [ ] Create MCP tool: `synap_sortedset_zinterstore`

## Phase 5: Persistence Integration (Week 5)

### WAL Operations
- [ ] Add `ZAdd` variant to `Operation` enum
- [ ] Add `ZRem` variant to `Operation` enum
- [ ] Add `ZIncrBy` variant to `Operation` enum
- [ ] Add `ZRemRange` variant to `Operation` enum
- [ ] Implement `PersistenceLayer::log_zadd`
- [ ] Implement `PersistenceLayer::log_zrem`
- [ ] Implement `PersistenceLayer::log_zincrby`
- [ ] Implement `PersistenceLayer::log_zremrange`
- [ ] Update `recover()` function to handle SortedSet operations
- [ ] Add SortedSet recovery from WAL replay
- [ ] Initialize `SortedSetStore` from snapshot data
- [ ] Integrate SortedSet with `AppState`
- [ ] Write 10+ persistence tests

## Phase 6: Testing (Week 5-6)

### Unit Tests (25+ tests)
- [ ] test_zadd_single
- [ ] test_zadd_multiple
- [ ] test_zadd_update
- [ ] test_zadd_nx_option
- [ ] test_zadd_xx_option
- [ ] test_zadd_gt_option
- [ ] test_zadd_lt_option
- [ ] test_zrem_single
- [ ] test_zrem_multiple
- [ ] test_zscore
- [ ] test_zcard
- [ ] test_zincrby
- [ ] test_zrange_basic
- [ ] test_zrange_withscores
- [ ] test_zrevrange
- [ ] test_zrangebyscore
- [ ] test_zrank
- [ ] test_zrevrank
- [ ] test_zcount
- [ ] test_zpopmin
- [ ] test_zpopmax
- [ ] test_zremrangebyrank
- [ ] test_zremrangebyscore
- [ ] test_zinterstore_basic
- [ ] test_zunionstore_weighted
- [ ] test_ttl_support

### Integration Tests (20+ tests)
- [ ] test_sortedset_rest_api_basic
- [ ] test_sortedset_streamable_http
- [ ] test_sortedset_range_operations
- [ ] test_sortedset_ranking
- [ ] test_sortedset_pop_operations
- [ ] test_sortedset_set_operations
- [ ] test_sortedset_weighted_union
- [ ] test_sortedset_persistence_recovery
- [ ] test_sortedset_concurrent_access
- [ ] test_sortedset_large_dataset
- [ ] test_sortedset_score_precision
- [ ] test_sortedset_edge_cases
- [ ] test_sortedset_with_replication
- [ ] test_sortedset_mcp_tools
- [ ] test_sortedset_blocking_ops
- [ ] test_sortedset_scan
- [ ] test_sortedset_lex_range
- [ ] test_sortedset_aggregate_min
- [ ] test_sortedset_aggregate_max
- [ ] test_sortedset_negative_scores

## Phase 7: Performance Benchmarking (Week 6)

### Benchmarks (15+ scenarios)
- [ ] zadd_single_element
- [ ] zadd_100_elements
- [ ] zadd_1000_elements
- [ ] zadd_10000_elements
- [ ] zrem_single
- [ ] zscore_lookup
- [ ] zrange_100_elements
- [ ] zrange_1000_elements
- [ ] zrangebyscore_various_ranges
- [ ] zrank_various_positions
- [ ] zpopmin_single
- [ ] zpopmax_single
- [ ] zinterstore_2_sets
- [ ] zunionstore_3_sets_weighted
- [ ] concurrent_zadd (100 threads)
- [ ] large_sorted_set (100K members)
- [ ] Verify all benchmarks meet performance targets

## Phase 8: Documentation (Week 6)

- [ ] Update `README.md` with Sorted Set capabilities
- [ ] Add Sorted Set examples to README
- [ ] Update feature comparison table
- [ ] Update MCP tool count
- [ ] Create CHANGELOG entry for v0.7.0-alpha
- [ ] Document Sorted Set API in CHANGELOG
- [ ] Update `docs/api/openapi.yml` with Sorted Set endpoints
- [ ] Add code comments for public Sorted Set APIs
- [ ] Document persistence integration
- [ ] Document weighted set operations
- [ ] Create usage examples (leaderboard, time-series)

## 📊 Summary

- **Total Tasks**: ~200
- **Estimated Duration**: 6 weeks
- **Dependencies**: Hash, List, Set (ALL COMPLETE ✅)
- **Complexity**: High (dual data structure, weighted operations)

## 🎯 Success Criteria

- [ ] All 25+ sorted set commands functional
- [ ] Weighted set operations working correctly
- [ ] Performance targets met (ZADD <200µs, ZRANGE <1ms)
- [ ] 95%+ test coverage
- [ ] Zero clippy warnings
- [ ] All integration tests passing
- [ ] OpenAPI spec updated
- [ ] Ready for v0.7.0-alpha release

## 🚀 Performance Targets

- **ZADD**: < 200µs
- **ZSCORE**: < 50µs
- **ZRANGE** (100 items): < 1ms
- **ZRANK**: < 100µs
- **ZINTERSTORE** (2 sets, 1K members each): < 5ms
- **Memory overhead**: < 40 bytes per member
- **Concurrent operations**: Linear scaling with cores

## 📖 Use Cases

- **Leaderboards**: Gaming scores, ranking systems
- **Priority Queues**: Task scheduling, job queues
- **Rate Limiting**: Token bucket with timestamps
- **Time-Series**: Temporal data with score = timestamp
- **Auto-Complete**: Prefix matching with scores
- **Geospatial**: Location-based queries (with geohash scores)
