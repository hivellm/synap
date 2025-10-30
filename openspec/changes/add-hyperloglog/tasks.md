# Tasks: Add HyperLogLog Support

> **Status**: 📋 Pending  
> **Target**: v0.8.0-alpha  
> **Priority**: Medium (Phase 4)

## Core (3 commands, ~40 tasks, 2 weeks)

### Implementation
- [ ] HyperLogLog storage (use `hyperloglog` crate)
- [ ] PFADD, PFCOUNT, PFMERGE
- [ ] 10+ unit tests

### API
- [ ] 3 REST endpoints, 3 StreamableHTTP commands

### Testing
- [ ] 12+ unit tests, 8+ integration tests (accuracy validation)

