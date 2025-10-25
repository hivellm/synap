# Tasks: Add Lua Scripting Support

## Core (6 commands, ~150 tasks, 8 weeks)

### Implementation
- [ ] Embed mlua interpreter
- [ ] Script caching with SHA1
- [ ] Sandboxing (disable dangerous functions)
- [ ] EVAL, EVALSHA, SCRIPT LOAD/EXISTS/FLUSH/KILL
- [ ] Timeout enforcement (tokio::time::timeout)
- [ ] redis.call() bridge to Synap commands
- [ ] 25+ unit tests

### API
- [ ] 6 REST endpoints, 6 StreamableHTTP commands, 2 MCP tools

### Testing
- [ ] 30+ unit tests, 15+ integration tests (rate limiting, complex logic)

### Performance Targets
- [ ] Script compilation <10ms, cached execution <1ms overhead

