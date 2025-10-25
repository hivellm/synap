# Tasks: Add Key Management Commands

## Core (6 commands, ~60 tasks, 2 weeks)

### Implementation
- [ ] KeyManager module with EXISTS, TYPE, RENAME, RENAMENX, COPY, RANDOMKEY
- [ ] Multi-type support (detect KV/Hash/List/Set/SortedSet)
- [ ] Atomic RENAME, COPY operations
- [ ] 12+ unit tests

### API
- [ ] 6 REST endpoints, 6 StreamableHTTP commands, 3 MCP tools

### Testing
- [ ] 15+ unit tests, 10+ integration tests, 6 benchmarks

### Performance Targets
- [ ] All operations <200µs

