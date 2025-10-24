# Add Set Data Structure

> **Status**: Draft  
> **Created**: 2025-10-24  
> **Priority**: High (Phase 1)  
> **Target Version**: v0.5.0-alpha

## Why

Sets are essential for unique collections, tags, relationships, and set algebra operations. They enable unique visitor tracking, tag systems, follower relationships, and membership tests - critical for modern applications.

**Problem**: Synap lacks Redis-compatible Set operations for unique collection management and set algebra (union, intersection, difference).

## What Changes

Implement Redis-compatible Set data structure with 15+ commands:

### Basic Operations
- `SADD` - Add members
- `SREM` - Remove members
- `SISMEMBER` - Check membership
- `SMEMBERS` - Get all members
- `SCARD` - Count members
- `SPOP` - Remove random member(s)
- `SRANDMEMBER` - Get random member(s)
- `SMOVE` - Move member between sets

### Set Algebra Operations
- `SINTER` - Intersection of sets
- `SUNION` - Union of sets
- `SDIFF` - Difference of sets
- `SINTERSTORE` - Store intersection result
- `SUNIONSTORE` - Store union result
- `SDIFFSTORE` - Store difference result

### Advanced
- `SSCAN` - Iterate set members
- `SINTERCARD` - Count intersection size

### API Layers
- **REST API**: 15 endpoints
- **StreamableHTTP**: 15 commands
- **MCP Tools**: 5 tools

### Persistence
- WAL integration (SetAdd, SetRem, SetMove operations)
- Recovery from WAL
- TTL support on entire set

## Impact

**Affected Specs**:
- NEW: `openspec/specs/set-store/spec.md`

**Affected Code**:
- `synap-server/src/core/set.rs` (NEW - ~700 lines)
- `synap-server/src/server/handlers.rs` (+400 lines)
- `synap-server/src/server/router.rs` (+15 routes)
- `synap-server/src/persistence/types.rs` (+3 Operation variants)
- `synap-server/tests/set_integration_tests.rs` (NEW - ~800 lines)

**Breaking Changes**: None (additive only)

## Success Criteria

- [ ] 15+ set commands implemented
- [ ] Set algebra operations (SINTER, SUNION, SDIFF)
- [ ] SINTERSTORE/SUNIONSTORE/SDIFFSTORE atomic operations
- [ ] REST API + StreamableHTTP + MCP coverage
- [ ] WAL persistence integration
- [ ] 95%+ test coverage
- [ ] Performance targets met:
  - SADD < 100µs
  - SISMEMBER < 50µs
  - SMEMBERS (1K items) < 1ms
  - SINTER (2 sets, 10K items) < 5ms

## Risks

**Low-Medium Complexity**:
- Set algebra needs multi-key read locks
- SINTERSTORE needs atomic cross-key writes
- Random operations need efficient sampling

**Mitigation**:
- Use `HashSet<Value>` internally for O(1) operations
- Implement multi-key locking with sorted key order (avoid deadlocks)
- Optimize intersection by iterating smallest set first

## Dependencies

- Requires Hash implementation (DONE ✅)
- Can run in parallel with Lists
- Enables: Set-based recommendations, tag systems

## Timeline

**Estimated Duration**: 2-3 weeks

- Week 1: Core implementation + basic ops + unit tests
- Week 2: Set algebra operations + atomic store ops
- Week 3: API layers + integration tests + benchmarks

