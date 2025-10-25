# Tasks: Add Enhanced Monitoring Commands

## Core (8 commands, ~80 tasks, 3 weeks)

### Implementation
- [ ] ServerInfo struct (version, uptime, memory, stats, replication)
- [ ] Slow query logging with configurable threshold
- [ ] MEMORY USAGE per key tracking
- [ ] CLIENT LIST connection tracking
- [ ] 15+ unit tests

### API
- [ ] 8 REST endpoints (GET /info, /info/server, /info/memory, /slowlog, etc.)
- [ ] 8 StreamableHTTP commands

### Testing
- [ ] 18+ unit tests, 12+ integration tests

### Performance Targets
- [ ] INFO command <1ms, SLOWLOG <500µs

