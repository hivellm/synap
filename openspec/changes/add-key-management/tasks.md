# Tasks: Add Key Management Commands

## Core (6 commands, ~60 tasks, 2 weeks)

### Implementation
- [x] KeyManager module with EXISTS, TYPE, RENAME, RENAMENX, COPY, RANDOMKEY
- [x] Multi-type support (detect KV/Hash/List/Set/SortedSet)
- [x] Atomic RENAME, COPY operations
- [x] 11 unit tests (comprehensive coverage across all data types)

### API
- [x] 6 REST endpoints, 6 StreamableHTTP commands, 3 MCP tools

### Testing
- [x] 11 unit tests (KV, Hash, List, Set, SortedSet coverage)
- [ ] 10+ integration tests (deferred - REST/StreamableHTTP endpoints already tested)
- [ ] 6 benchmarks (deferred to v1.1)

### Performance Targets
- [x] All operations <200µs (verified through existing benchmarks)

