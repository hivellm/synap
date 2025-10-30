# Tasks: Add Transactions Support

> **Status**: ✅ Complete  
> **Target**: v0.7.0-alpha  
> **Priority**: High (Phase 3)  
> **Progress**: 100% (Core features + integration tests complete, performance benchmarks optional)

## Core (5 commands, ~120 tasks, 6 weeks)

### Implementation
- [x] Transaction struct with command queue
- [x] Key versioning system (VersionedValue) for WATCH
- [x] MULTI, EXEC, DISCARD, WATCH, UNWATCH
- [x] Multi-key locking (sorted to avoid deadlock)
- [x] Conflict detection and rollback (optimistic locking)
- [x] 11 unit tests (basic coverage)

### API
- [x] 5 REST endpoints: POST /transaction/{multi,exec,discard,watch,unwatch}
- [x] 5 StreamableHTTP commands: transaction.multi, transaction.exec, transaction.discard, transaction.watch, transaction.unwatch
- [x] 2 MCP tools: synap_transaction_multi, synap_transaction_exec

### Testing
- [x] 11 unit tests (MULTI/DISCARD, queue commands, WATCH/UNWATCH, error cases)
- [x] 3 integration tests (REST API endpoints: MULTI/EXEC, DISCARD, WATCH/UNWATCH)

### Performance Targets
- [ ] Transaction overhead <500µs (not yet benchmarked)
- [ ] WATCH <100µs/key (not yet benchmarked)

### Notes
- Core transaction functionality fully implemented
- WATCH uses optimistic locking with key versioning
- Support for KV SET/DEL/INCR commands in transactions (more commands can be added)
- All test helpers updated with TransactionManager
- MCP tools configured but disabled by default (enable_transaction_tools: false)

