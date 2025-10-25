# Add Transactions Support

> **Status**: Draft  
> **Priority**: High (Phase 3)  
> **Target**: v0.7.0-alpha  
> **Duration**: 6 weeks

## Why

Atomic multi-key operations essential for financial transactions, inventory management, and optimistic locking patterns.

## What Changes

Implement Redis MULTI/EXEC/WATCH/DISCARD with optimistic locking:

**Commands**: MULTI, EXEC, DISCARD, WATCH, UNWATCH

**Features**:
- Transaction context per client
- Key versioning for WATCH
- Atomic execution
- Rollback on conflict

**API**: REST (5 endpoints) + StreamableHTTP (5 commands) + MCP (2 tools)

## Impact

**NEW**: `synap-server/src/core/transaction.rs` (~800 lines)  
**Complexity**: High (versioning, multi-key locks, deadlock prevention)  
**Performance**: Transaction overhead <500µs, WATCH <100µs/key

