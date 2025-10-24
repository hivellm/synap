# Tasks: Add Transactions Support

## Core (5 commands, ~120 tasks, 6 weeks)

### Implementation
- [ ] Transaction struct with command queue
- [ ] Key versioning system (VersionedValue)
- [ ] MULTI, EXEC, DISCARD, WATCH, UNWATCH
- [ ] Multi-key locking (sorted to avoid deadlock)
- [ ] Conflict detection and rollback
- [ ] 20+ unit tests

### API
- [ ] 5 REST endpoints, 5 StreamableHTTP commands, 2 MCP tools

### Testing
- [ ] 25+ unit tests, 15+ integration tests (atomic ops, conflict detection)

### Performance Targets
- [ ] Transaction overhead <500µs, WATCH <100µs/key

