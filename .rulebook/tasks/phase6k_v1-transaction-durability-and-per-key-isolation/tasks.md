## 1. Implementation
- [ ] 1.1 Have TransactionManager::exec return the executed commands to the caller
- [ ] 1.2 Add a synap-server helper mapping TransactionCommand -> persistence Operation and logging it (deterministic effect for INCR/POP)
- [ ] 1.3 Call the helper from all five EXEC entry points (RESP3, SynapRPC, admin, kv, mcp)
- [ ] 1.4 Add a WAL batch API so an EXEC is logged as one atomic unit
- [ ] 1.5 Introduce per-key locking so EXEC is isolated from non-transactional writers
- [ ] 1.6 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior (EXEC durable after crash; EXEC replicated; isolation from concurrent writer)
- [ ] 2.3 Run tests and confirm they pass
