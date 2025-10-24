# Tasks: Add List Data Structure

## Phase 1: Core Implementation

- [ ] Create `ListStore` struct with 64-way sharding
- [ ] Implement `ListValue` with VecDeque storage
- [ ] Add `LPUSH` operation
- [ ] Add `RPUSH` operation
- [ ] Add `LPOP` operation
- [ ] Add `RPOP` operation
- [ ] Add `LRANGE` operation
- [ ] Add `LLEN` operation
- [ ] Add `LINDEX` operation
- [ ] Add `LSET` operation
- [ ] Add `LPUSHX` operation (push if exists)
- [ ] Add `RPUSHX` operation (push if exists)
- [ ] Implement TTL support for entire list
- [ ] Add statistics tracking (`ListStats`)
- [ ] Write 15+ unit tests for list module

## Phase 2: Advanced Operations

- [ ] Add `LTRIM` operation
- [ ] Add `LREM` operation (remove by value)
- [ ] Add `LINSERT` operation (insert before/after)
- [ ] Add `RPOPLPUSH` operation (atomic move)
- [ ] Add `LPOS` operation (find position)
- [ ] Write 10+ tests for advanced operations

## Phase 3: Blocking Operations

- [ ] Implement blocking channel infrastructure
- [ ] Add `BLPOP` operation (blocking pop left)
- [ ] Add `BRPOP` operation (blocking pop right)
- [ ] Add `BRPOPLPUSH` operation (blocking move)
- [ ] Add timeout support for blocking ops
- [ ] Add multi-key blocking support
- [ ] Write 8+ tests for blocking operations

## Phase 4: API Exposure

### REST API
- [ ] Create REST endpoint: `POST /list/{key}/lpush`
- [ ] Create REST endpoint: `POST /list/{key}/rpush`
- [ ] Create REST endpoint: `POST /list/{key}/lpop`
- [ ] Create REST endpoint: `POST /list/{key}/rpop`
- [ ] Create REST endpoint: `GET /list/{key}/range`
- [ ] Create REST endpoint: `GET /list/{key}/len`
- [ ] Create REST endpoint: `GET /list/{key}/index`
- [ ] Create REST endpoint: `POST /list/{key}/set`
- [ ] Create REST endpoint: `POST /list/{key}/trim`
- [ ] Create REST endpoint: `POST /list/{key}/rem`
- [ ] Create REST endpoint: `POST /list/{key}/insert`
- [ ] Create REST endpoint: `POST /list/{key}/rpoplpush`
- [ ] Create REST endpoint: `POST /list/{key}/blpop`
- [ ] Create REST endpoint: `POST /list/{key}/brpop`
- [ ] Create REST endpoint: `POST /list/{key}/brpoplpush`
- [ ] Create REST endpoint: `GET /list/stats`

### StreamableHTTP
- [ ] Add StreamableHTTP command: `list.lpush`
- [ ] Add StreamableHTTP command: `list.rpush`
- [ ] Add StreamableHTTP command: `list.lpop`
- [ ] Add StreamableHTTP command: `list.rpop`
- [ ] Add StreamableHTTP command: `list.range`
- [ ] Add StreamableHTTP command: `list.len`
- [ ] Add StreamableHTTP command: `list.index`
- [ ] Add StreamableHTTP command: `list.set`
- [ ] Add StreamableHTTP command: `list.trim`
- [ ] Add StreamableHTTP command: `list.rem`
- [ ] Add StreamableHTTP command: `list.insert`
- [ ] Add StreamableHTTP command: `list.rpoplpush`
- [ ] Add StreamableHTTP command: `list.blpop`
- [ ] Add StreamableHTTP command: `list.brpop`
- [ ] Add StreamableHTTP command: `list.brpoplpush`
- [ ] Add StreamableHTTP command: `list.stats`

### MCP Tools
- [ ] Create MCP tool: `synap_list_push`
- [ ] Create MCP tool: `synap_list_pop`
- [ ] Create MCP tool: `synap_list_range`
- [ ] Create MCP tool: `synap_list_len`
- [ ] Create MCP tool: `synap_list_rpoplpush`

## Phase 5: Persistence Integration

- [ ] Add `ListPush` variant to `Operation` enum
- [ ] Add `ListPop` variant to `Operation` enum
- [ ] Add `ListSet` variant to `Operation` enum
- [ ] Add `ListTrim` variant to `Operation` enum
- [ ] Add `ListRem` variant to `Operation` enum
- [ ] Add `ListInsert` variant to `Operation` enum
- [ ] Implement `PersistenceLayer::log_list_push`
- [ ] Implement `PersistenceLayer::log_list_pop`
- [ ] Implement `PersistenceLayer::log_list_set`
- [ ] Implement `PersistenceLayer::log_list_trim`
- [ ] Update `recover()` function to handle List operations
- [ ] Add List recovery from WAL replay
- [ ] Initialize `ListStore` from snapshot data
- [ ] Integrate List with `AppState`

## Phase 6: Testing

### Unit Tests
- [ ] Unit test: `test_lpush_rpush`
- [ ] Unit test: `test_lpop_rpop`
- [ ] Unit test: `test_lrange_all`
- [ ] Unit test: `test_lrange_partial`
- [ ] Unit test: `test_llen`
- [ ] Unit test: `test_lindex_valid`
- [ ] Unit test: `test_lindex_out_of_bounds`
- [ ] Unit test: `test_lset`
- [ ] Unit test: `test_ltrim`
- [ ] Unit test: `test_lrem_count_positive`
- [ ] Unit test: `test_lrem_count_negative`
- [ ] Unit test: `test_lrem_count_zero`
- [ ] Unit test: `test_linsert_before`
- [ ] Unit test: `test_linsert_after`
- [ ] Unit test: `test_rpoplpush`
- [ ] Unit test: `test_lpushx_exists`
- [ ] Unit test: `test_lpushx_not_exists`

### Integration Tests
- [ ] Integration test: `test_list_push_pop_rest`
- [ ] Integration test: `test_list_range_rest`
- [ ] Integration test: `test_list_trim_rest`
- [ ] Integration test: `test_list_rem_rest`
- [ ] Integration test: `test_list_insert_rest`
- [ ] Integration test: `test_list_rpoplpush_rest`
- [ ] Integration test: `test_list_blpop_timeout`
- [ ] Integration test: `test_list_blpop_immediate`
- [ ] Integration test: `test_list_blpop_multiple_keys`
- [ ] Integration test: `test_list_streamable_http`
- [ ] Integration test: `test_list_with_persistence_recovery`
- [ ] Integration test: `test_list_concurrent_access`
- [ ] Integration test: `test_list_large_elements`
- [ ] Integration test: `test_list_edge_cases`
- [ ] Fix all AppState compatibility tests

## Phase 7: Performance Benchmarking

- [ ] Benchmark: `lpush_single_element`
- [ ] Benchmark: `lpush_100_elements`
- [ ] Benchmark: `lpush_1000_elements`
- [ ] Benchmark: `rpush_single_element`
- [ ] Benchmark: `lpop_single`
- [ ] Benchmark: `rpop_single`
- [ ] Benchmark: `lrange_100_elements`
- [ ] Benchmark: `lrange_1000_elements`
- [ ] Benchmark: `lindex_various_positions`
- [ ] Benchmark: `lset_various_positions`
- [ ] Benchmark: `ltrim_various_sizes`
- [ ] Benchmark: `lrem_various_counts`
- [ ] Benchmark: `rpoplpush_operations`
- [ ] Benchmark: `blpop_no_wait`
- [ ] Benchmark: Concurrent access (100 threads)
- [ ] Verify all benchmarks meet performance targets

## Phase 8: Documentation

- [ ] Update `README.md` with List capabilities
- [ ] Add List examples to README
- [ ] Update feature comparison table
- [ ] Update MCP tool count
- [ ] Create CHANGELOG entry for v0.5.0-alpha
- [ ] Document List API in CHANGELOG
- [ ] Update `docs/api/openapi.yml` with List endpoints
- [ ] Add code comments for public List APIs
- [ ] Document persistence integration
- [ ] Document blocking operations behavior

## 📊 Summary

- **Total Tasks**: ~150
- **Estimated Duration**: 3-4 weeks
- **Dependencies**: Hash (DONE ✅)
- **Complexity**: Medium-High (blocking ops)

## 🎯 Success Criteria

- [ ] All 16+ list commands functional
- [ ] Blocking operations working correctly
- [ ] Performance targets met
- [ ] 95%+ test coverage
- [ ] Zero clippy warnings
- [ ] All integration tests passing
- [ ] OpenAPI spec updated
- [ ] Ready for v0.5.0-alpha release

