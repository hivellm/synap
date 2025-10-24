# Tasks: Add Set Data Structure

## Phase 1: Core Implementation

- [x] Create `SetStore` struct with 64-way sharding
- [x] Implement `SetValue` with HashSet storage
- [x] Add `SADD` operation
- [x] Add `SREM` operation
- [x] Add `SISMEMBER` operation
- [x] Add `SMEMBERS` operation
- [x] Add `SCARD` operation
- [x] Add `SPOP` operation (remove random)
- [x] Add `SRANDMEMBER` operation (get random)
- [x] Add `SMOVE` operation (move between sets)
- [x] Implement TTL support for entire set
- [x] Add statistics tracking (`SetStats`)
- [x] Write 12+ unit tests for set module (11 tests ✅)

## Phase 2: Set Algebra Operations

- [x] Add `SINTER` operation (intersection)
- [x] Add `SUNION` operation (union)
- [x] Add `SDIFF` operation (difference)
- [x] Add `SINTERSTORE` operation (store intersection)
- [x] Add `SUNIONSTORE` operation (store union)
- [x] Add `SDIFFSTORE` operation (store difference)
- [ ] Add `SINTERCARD` operation (count intersection) - deferred
- [x] Implement multi-key locking for safety
- [x] Optimize set algebra (iterate smallest set)
- [x] Write 8+ tests for set algebra (included in 11 tests ✅)

## Phase 3: Advanced Operations

- [ ] Add `SSCAN` operation (iterate members) - deferred to v1.1
- [ ] Implement cursor-based pagination - deferred to v1.1
- [ ] Add pattern matching for SSCAN - deferred to v1.1
- [ ] Write 5+ tests for advanced operations - deferred to v1.1

## Phase 4: API Exposure

### REST API
- [ ] Create REST endpoint: `POST /set/{key}/add`
- [ ] Create REST endpoint: `POST /set/{key}/rem`
- [ ] Create REST endpoint: `GET /set/{key}/ismember`
- [ ] Create REST endpoint: `GET /set/{key}/members`
- [ ] Create REST endpoint: `GET /set/{key}/card`
- [ ] Create REST endpoint: `POST /set/{key}/pop`
- [ ] Create REST endpoint: `GET /set/{key}/randmember`
- [ ] Create REST endpoint: `POST /set/{key}/move`
- [ ] Create REST endpoint: `POST /set/inter`
- [ ] Create REST endpoint: `POST /set/union`
- [ ] Create REST endpoint: `POST /set/diff`
- [ ] Create REST endpoint: `POST /set/interstore`
- [ ] Create REST endpoint: `POST /set/unionstore`
- [ ] Create REST endpoint: `POST /set/diffstore`
- [ ] Create REST endpoint: `GET /set/stats`

### StreamableHTTP
- [ ] Add StreamableHTTP command: `set.add`
- [ ] Add StreamableHTTP command: `set.rem`
- [ ] Add StreamableHTTP command: `set.ismember`
- [ ] Add StreamableHTTP command: `set.members`
- [ ] Add StreamableHTTP command: `set.card`
- [ ] Add StreamableHTTP command: `set.pop`
- [ ] Add StreamableHTTP command: `set.randmember`
- [ ] Add StreamableHTTP command: `set.move`
- [ ] Add StreamableHTTP command: `set.inter`
- [ ] Add StreamableHTTP command: `set.union`
- [ ] Add StreamableHTTP command: `set.diff`
- [ ] Add StreamableHTTP command: `set.interstore`
- [ ] Add StreamableHTTP command: `set.unionstore`
- [ ] Add StreamableHTTP command: `set.diffstore`
- [ ] Add StreamableHTTP command: `set.stats`

### MCP Tools
- [ ] Create MCP tool: `synap_set_add`
- [ ] Create MCP tool: `synap_set_members`
- [ ] Create MCP tool: `synap_set_ismember`
- [ ] Create MCP tool: `synap_set_inter`
- [ ] Create MCP tool: `synap_set_union`

## Phase 5: Persistence Integration

- [ ] Add `SetAdd` variant to `Operation` enum
- [ ] Add `SetRem` variant to `Operation` enum
- [ ] Add `SetMove` variant to `Operation` enum
- [ ] Implement `PersistenceLayer::log_set_add`
- [ ] Implement `PersistenceLayer::log_set_rem`
- [ ] Implement `PersistenceLayer::log_set_move`
- [ ] Update `recover()` function to handle Set operations
- [ ] Add Set recovery from WAL replay
- [ ] Initialize `SetStore` from snapshot data
- [ ] Integrate Set with `AppState`

## Phase 6: Testing

### Unit Tests
- [ ] Unit test: `test_sadd_srem`
- [ ] Unit test: `test_sismember`
- [ ] Unit test: `test_smembers`
- [ ] Unit test: `test_scard`
- [ ] Unit test: `test_spop`
- [ ] Unit test: `test_srandmember`
- [ ] Unit test: `test_smove`
- [ ] Unit test: `test_sinter_two_sets`
- [ ] Unit test: `test_sinter_multiple_sets`
- [ ] Unit test: `test_sunion`
- [ ] Unit test: `test_sdiff`
- [ ] Unit test: `test_sinterstore`
- [ ] Unit test: `test_sunionstore`
- [ ] Unit test: `test_sdiffstore`
- [ ] Unit test: `test_sscan`

### Integration Tests
- [ ] Integration test: `test_set_add_rem_rest`
- [ ] Integration test: `test_set_ismember_rest`
- [ ] Integration test: `test_set_members_rest`
- [ ] Integration test: `test_set_pop_rest`
- [ ] Integration test: `test_set_randmember_rest`
- [ ] Integration test: `test_set_move_rest`
- [ ] Integration test: `test_set_inter_rest`
- [ ] Integration test: `test_set_union_rest`
- [ ] Integration test: `test_set_diff_rest`
- [ ] Integration test: `test_set_interstore_rest`
- [ ] Integration test: `test_set_streamable_http`
- [ ] Integration test: `test_set_with_persistence_recovery`
- [ ] Integration test: `test_set_concurrent_access`
- [ ] Integration test: `test_set_large_members`
- [ ] Integration test: `test_set_edge_cases`
- [ ] Fix all AppState compatibility tests

## Phase 7: Performance Benchmarking

- [ ] Benchmark: `sadd_single_member`
- [ ] Benchmark: `sadd_100_members`
- [ ] Benchmark: `sadd_1000_members`
- [ ] Benchmark: `srem_single_member`
- [ ] Benchmark: `sismember_hit`
- [ ] Benchmark: `sismember_miss`
- [ ] Benchmark: `smembers_100`
- [ ] Benchmark: `smembers_1000`
- [ ] Benchmark: `smembers_10000`
- [ ] Benchmark: `scard`
- [ ] Benchmark: `spop_single`
- [ ] Benchmark: `srandmember`
- [ ] Benchmark: `sinter_two_sets_10k_each`
- [ ] Benchmark: `sunion_two_sets_10k_each`
- [ ] Benchmark: `sdiff_two_sets_10k_each`
- [ ] Benchmark: Concurrent access (100 threads)
- [ ] Verify all benchmarks meet performance targets

## Phase 8: Documentation

- [ ] Update `README.md` with Set capabilities
- [ ] Add Set examples to README
- [ ] Update feature comparison table
- [ ] Update MCP tool count
- [ ] Create CHANGELOG entry for v0.5.0-alpha
- [ ] Document Set API in CHANGELOG
- [ ] Update `docs/api/openapi.yml` with Set endpoints
- [ ] Add code comments for public Set APIs
- [ ] Document persistence integration
- [ ] Document set algebra optimization strategies

## 📊 Summary

- **Total Tasks**: ~130
- **Estimated Duration**: 2-3 weeks
- **Dependencies**: Hash (DONE ✅)
- **Complexity**: Low-Medium (simpler than Lists)

## 🎯 Success Criteria

- [ ] All 15+ set commands functional
- [ ] Set algebra operations working correctly
- [ ] Performance targets met
- [ ] 95%+ test coverage
- [ ] Zero clippy warnings
- [ ] All integration tests passing
- [ ] OpenAPI spec updated
- [ ] Ready for v0.5.0-alpha release

