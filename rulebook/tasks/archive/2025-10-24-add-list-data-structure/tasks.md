# Tasks: Add List Data Structure

## Phase 1: Core Implementation

- [x] Create `ListStore` struct with 64-way sharding
- [x] Implement `ListValue` with VecDeque storage
- [x] Add `LPUSH` operation
- [x] Add `RPUSH` operation
- [x] Add `LPOP` operation
- [x] Add `RPOP` operation
- [x] Add `LRANGE` operation
- [x] Add `LLEN` operation
- [x] Add `LINDEX` operation
- [x] Add `LSET` operation
- [x] Add `LPUSHX` operation (push if exists)
- [x] Add `RPUSHX` operation (push if exists)
- [x] Implement TTL support for entire list
- [x] Add statistics tracking (`ListStats`)
- [x] Write 15+ unit tests for list module (16 tests passing ✅)

## Phase 2: Advanced Operations

- [x] Add `LTRIM` operation
- [x] Add `LREM` operation (remove by value)
- [x] Add `LINSERT` operation (insert before/after)
- [x] Add `RPOPLPUSH` operation (atomic move)
- [x] Add `LPOS` operation (find position)
- [x] Write 10+ tests for advanced operations (included in 31 total tests ✅)

## Phase 3: Blocking Operations

- [x] Implement blocking channel infrastructure
- [x] Add `BLPOP` operation (blocking pop left)
- [x] Add `BRPOP` operation (blocking pop right)
- [x] Add `BRPOPLPUSH` operation (blocking move)
- [x] Add timeout support for blocking ops
- [x] Add multi-key blocking support
- [x] Write 8+ tests for blocking operations

## Phase 4: API Exposure

### REST API
- [x] Create REST endpoint: `POST /list/{key}/lpush`
- [x] Create REST endpoint: `POST /list/{key}/rpush`
- [x] Create REST endpoint: `POST /list/{key}/lpop`
- [x] Create REST endpoint: `POST /list/{key}/rpop`
- [x] Create REST endpoint: `GET /list/{key}/range`
- [x] Create REST endpoint: `GET /list/{key}/len`
- [x] Create REST endpoint: `GET /list/{key}/index`
- [x] Create REST endpoint: `POST /list/{key}/set`
- [x] Create REST endpoint: `POST /list/{key}/trim`
- [x] Create REST endpoint: `POST /list/{key}/rem`
- [x] Create REST endpoint: `POST /list/{key}/insert`
- [x] Create REST endpoint: `POST /list/{key}/rpoplpush`
- [x] Create REST endpoint: `POST /list/{key}/blpop`
- [x] Create REST endpoint: `POST /list/{key}/brpop`
- [x] Create REST endpoint: `POST /list/{key}/brpoplpush`
- [x] Create REST endpoint: `GET /list/stats`

### StreamableHTTP
- [x] Add StreamableHTTP command: `list.lpush`
- [x] Add StreamableHTTP command: `list.rpush`
- [x] Add StreamableHTTP command: `list.lpop`
- [x] Add StreamableHTTP command: `list.rpop`
- [x] Add StreamableHTTP command: `list.range`
- [x] Add StreamableHTTP command: `list.len`
- [x] Add StreamableHTTP command: `list.index`
- [x] Add StreamableHTTP command: `list.set`
- [x] Add StreamableHTTP command: `list.trim`
- [x] Add StreamableHTTP command: `list.rem`
- [x] Add StreamableHTTP command: `list.insert`
- [x] Add StreamableHTTP command: `list.rpoplpush`
- [x] Add StreamableHTTP command: `list.blpop`
- [x] Add StreamableHTTP command: `list.brpop`
- [x] Add StreamableHTTP command: `list.brpoplpush`
- [x] Add StreamableHTTP command: `list.stats`

### MCP Tools
- [x] Create MCP tool: `synap_list_push`
- [x] Create MCP tool: `synap_list_pop`
- [x] Create MCP tool: `synap_list_range`
- [x] Create MCP tool: `synap_list_len`
- [x] Create MCP tool: `synap_list_rpoplpush`

## Phase 5: Persistence Integration

- [x] Add `ListPush` variant to `Operation` enum
- [x] Add `ListPop` variant to `Operation` enum
- [x] Add `ListSet` variant to `Operation` enum
- [x] Add `ListTrim` variant to `Operation` enum
- [x] Add `ListRem` variant to `Operation` enum
- [x] Add `ListInsert` variant to `Operation` enum
- [x] Implement `PersistenceLayer::log_list_push`
- [x] Implement `PersistenceLayer::log_list_pop`
- [x] Implement `PersistenceLayer::log_list_set`
- [x] Implement `PersistenceLayer::log_list_trim`
- [x] Update `recover()` function to handle List operations
- [x] Add List recovery from WAL replay
- [x] Initialize `ListStore` from snapshot data
- [x] Integrate List with `AppState`

## Phase 6: Testing

### Unit Tests
- [x] Unit test: `test_lpush_rpush`
- [x] Unit test: `test_lpop_rpop`
- [x] Unit test: `test_lrange_all`
- [x] Unit test: `test_lrange_partial`
- [x] Unit test: `test_llen`
- [x] Unit test: `test_lindex_valid`
- [x] Unit test: `test_lindex_out_of_bounds`
- [x] Unit test: `test_lset`
- [x] Unit test: `test_ltrim`
- [x] Unit test: `test_lrem_count_positive`
- [x] Unit test: `test_lrem_count_negative`
- [x] Unit test: `test_lrem_count_zero`
- [x] Unit test: `test_linsert_before`
- [x] Unit test: `test_linsert_after`
- [x] Unit test: `test_rpoplpush`
- [x] Unit test: `test_lpushx_exists`
- [x] Unit test: `test_lpushx_not_exists`

### Integration Tests
- [x] Integration test: `test_list_push_pop_rest`
- [x] Integration test: `test_list_range_rest`
- [x] Integration test: `test_list_trim_rest`
- [x] Integration test: `test_list_rem_rest`
- [x] Integration test: `test_list_insert_rest`
- [x] Integration test: `test_list_rpoplpush_rest`
- [x] Integration test: `test_list_blpop_timeout`
- [x] Integration test: `test_list_blpop_immediate`
- [x] Integration test: `test_list_blpop_multiple_keys`
- [x] Integration test: `test_list_streamable_http`
- [x] Integration test: `test_list_with_persistence_recovery`
- [x] Integration test: `test_list_concurrent_access`
- [x] Integration test: `test_list_large_elements`
- [x] Integration test: `test_list_edge_cases`
- [x] Fix all AppState compatibility tests

## Phase 7: Performance Benchmarking

- [x] Benchmark: `lpush_single_element`
- [x] Benchmark: `lpush_100_elements`
- [x] Benchmark: `lpush_1000_elements`
- [x] Benchmark: `rpush_single_element`
- [x] Benchmark: `lpop_single`
- [x] Benchmark: `rpop_single`
- [x] Benchmark: `lrange_100_elements`
- [x] Benchmark: `lrange_1000_elements`
- [x] Benchmark: `lindex_various_positions`
- [x] Benchmark: `lset_various_positions`
- [x] Benchmark: `ltrim_various_sizes`
- [x] Benchmark: `lrem_various_counts`
- [x] Benchmark: `rpoplpush_operations`
- [x] Benchmark: `blpop_no_wait`
- [x] Benchmark: Concurrent access (100 threads)
- [x] Verify all benchmarks meet performance targets

## Phase 8: Documentation

- [x] Update `README.md` with List capabilities
- [x] Add List examples to README
- [x] Update feature comparison table
- [x] Update MCP tool count
- [x] Create CHANGELOG entry for v0.5.0-alpha
- [x] Document List API in CHANGELOG
- [x] Update `docs/api/openapi.yml` with List endpoints
- [x] Add code comments for public List APIs
- [x] Document persistence integration
- [x] Document blocking operations behavior

## 📊 Summary

- **Total Tasks**: ~150
- **Estimated Duration**: 3-4 weeks
- **Dependencies**: Hash (DONE ✅)
- **Complexity**: Medium-High (blocking ops)

## 🎯 Success Criteria

- [x] All 16+ list commands functional
- [x] Blocking operations working correctly
- [x] Performance targets met
- [x] 95%+ test coverage
- [x] Zero clippy warnings
- [x] All integration tests passing
- [x] OpenAPI spec updated
- [x] Ready for v0.5.0-alpha release

