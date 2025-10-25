# Add Key Management Commands

> **Status**: Draft  
> **Priority**: Medium (Phase 2)  
> **Target**: v0.6.0-alpha  
> **Duration**: 2 weeks

## Why

Essential key operations missing: EXISTS, TYPE, RENAME, RENAMENX, COPY, RANDOMKEY.

## What Changes

Add 6 key management commands across all data types (KV, Hash, List, Set, SortedSet):

**Commands**: EXISTS, TYPE, RENAME, RENAMENX, COPY, RANDOMKEY

**API**: REST (6 endpoints) + StreamableHTTP (6 commands) + MCP (3 tools)

## Impact

**NEW**: `synap-server/src/core/key_manager.rs` (~400 lines)  
**Complexity**: Medium (multi-type support, atomic RENAME)  
**Performance**: All ops <200µs

