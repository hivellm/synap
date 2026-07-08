## 1. Implementation
- [ ] 1.1 Hold sorted per-key locks (or a serialized executor) across the whole execute_commands block
- [ ] 1.2 Make the WATCH version check and command apply one critical section (close the TOCTOU window)
- [ ] 1.3 Add a WAL batch API that logs all EXEC commands atomically as one unit
- [ ] 1.4 Route every transaction command through WAL/persistence and replication
- [ ] 1.5 Update transaction.rs module docs to match the real guarantees
- [ ] 1.6 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (transactions doc)
- [ ] 2.2 Write tests covering the new behavior (concurrent EXEC isolation; WATCH abort under concurrent write; EXEC durable after crash; EXEC replicated)
- [ ] 2.3 Run tests and confirm they pass
