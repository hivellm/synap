# Tasks: Add HyperLogLog Support

> **Status**: 🚧 In Progress (core ops shipped, unit/integration tests pending)  
> **Target**: v0.8.0-alpha  
> **Priority**: Medium (Phase 4)

## Core (3 commands, ~40 tasks, 2 weeks)

### Implementation
- [x] HyperLogLog storage (custom implementation using 16,384 registers)
- [x] PFADD, PFCOUNT, PFMERGE commands
- [ ] 10+ unit tests (currently 3)

### API
- [x] 3 REST endpoints, 3 StreamableHTTP commands

### Testing
- [ ] 12+ unit tests (currently 3)
- [ ] 8+ integration tests (currently 4)

