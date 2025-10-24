# Add List Data Structure

> **Status**: Draft  
> **Created**: 2025-10-24  
> **Priority**: High (Phase 1)  
> **Target Version**: v0.5.0-alpha

## Why

Lists are the second most critical Redis data structure after Hashes. They enable essential use cases like activity feeds, job queues, message buffers, and recent items caching. Without Lists, users cannot migrate from Redis or implement common patterns.

**Problem**: Synap has Queue (RabbitMQ-style) but lacks Redis-compatible List operations (LPUSH/RPUSH/LPOP/RPOP/LRANGE/etc).

## What Changes

Implement Redis-compatible List data structure with 16+ commands:

### Basic Operations
- `LPUSH` / `RPUSH` - Push to left/right
- `LPOP` / `RPOP` - Pop from left/right
- `LRANGE` - Get range of elements
- `LLEN` - Get list length
- `LINDEX` - Get element by index
- `LSET` - Set element by index

### Advanced Operations
- `LTRIM` - Keep only range
- `LREM` - Remove elements
- `LINSERT` - Insert before/after element
- `RPOPLPUSH` - Atomic move between lists

### Blocking Operations
- `BLPOP` / `BRPOP` - Blocking pop with timeout
- `BRPOPLPUSH` - Blocking atomic move

### API Layers
- **REST API**: 16 endpoints
- **StreamableHTTP**: 16 commands
- **MCP Tools**: 5 tools

### Persistence
- WAL integration (ListPush, ListPop, ListSet, ListTrim operations)
- Recovery from WAL
- TTL support on entire list

## Impact

**Affected Specs**:
- NEW: `openspec/specs/list-store/spec.md`

**Affected Code**:
- `synap-server/src/core/list.rs` (NEW - ~800 lines)
- `synap-server/src/server/handlers.rs` (+500 lines)
- `synap-server/src/server/router.rs` (+16 routes)
- `synap-server/src/persistence/types.rs` (+4 Operation variants)
- `synap-server/tests/list_integration_tests.rs` (NEW - ~900 lines)

**Breaking Changes**: None (additive only)

## Success Criteria

- [ ] 16+ list commands implemented
- [ ] Blocking operations with timeout support
- [ ] REST API + StreamableHTTP + MCP coverage
- [ ] WAL persistence integration
- [ ] 95%+ test coverage
- [ ] Performance targets met:
  - LPUSH/RPUSH < 100µs
  - LPOP/RPOP < 100µs
  - LRANGE (100 items) < 500µs
  - BLPOP (no wait) < 100µs

## Risks

**Medium Complexity**:
- Blocking operations require async channels
- Index-based access needs VecDeque optimization
- RPOPLPUSH needs atomic multi-key operation

**Mitigation**:
- Use `VecDeque<Value>` for O(1) push/pop at both ends
- Use `tokio::sync::broadcast` for blocking wait notifications
- Implement multi-key locks for atomic operations

## Dependencies

- Requires Hash implementation (DONE ✅)
- Blocks: Sets implementation (can run in parallel)
- Enables: Redis migration path

## Timeline

**Estimated Duration**: 3-4 weeks

- Week 1-2: Core implementation + unit tests
- Week 3: Advanced operations + blocking ops
- Week 4: API layers + integration tests + benchmarks

