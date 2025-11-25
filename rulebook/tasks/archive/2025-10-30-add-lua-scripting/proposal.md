# Add Lua Scripting Support

> **Status**: Draft  
> **Priority**: High (Phase 3)  
> **Target**: v0.7.0-alpha  
> **Duration**: 8 weeks

## Why

Server-side Lua scripting enables complex atomic operations, rate limiting, custom logic, and reduces network round-trips.

## What Changes

Implement Redis-compatible Lua scripting with `mlua` crate:

**Commands**: EVAL, EVALSHA, SCRIPT LOAD, SCRIPT EXISTS, SCRIPT FLUSH, SCRIPT KILL

**Features**:
- Embedded Lua interpreter
- Script caching by SHA1
- Sandboxing for security
- Timeout enforcement (5s default)
- `redis.call()` function support

**API**: REST (6 endpoints) + StreamableHTTP (6 commands) + MCP (2 tools)

## Impact

**NEW**: `synap-server/src/scripting/` (~1000 lines)  
**Complexity**: Very High (Lua embedding, sandboxing, timeout enforcement)  
**Performance**: Compilation <10ms, cached execution <1ms overhead

