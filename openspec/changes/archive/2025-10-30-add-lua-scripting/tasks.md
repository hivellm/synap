# Tasks: Add Lua Scripting Support

> **Status**: ✅ Complete  
> **Target**: v0.7.0-alpha  
> **Priority**: High (Phase 3)

## Core (6 commands, ~150 tasks, 8 weeks)

### Implementation
- [x] Embed mlua interpreter
- [x] Script caching with SHA1
- [x] Sandboxing (disable dangerous functions)
- [x] EVAL, EVALSHA, SCRIPT LOAD/EXISTS/FLUSH/KILL
- [x] Timeout enforcement (tokio::time::timeout)
- [x] redis.call() bridge to Synap commands
- [x] 25+ unit tests

### API
- [x] 6 REST endpoints, 6 StreamableHTTP commands, 2 MCP tools

### Testing
- [x] 30+ unit tests, 15+ integration tests (rate limiting, complex logic)

### Performance Targets
- [ ] Script compilation <10ms, cached execution <1ms overhead

