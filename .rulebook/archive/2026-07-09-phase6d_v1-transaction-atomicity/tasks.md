## 1. Implementation
- [x] 1.1 Serialize EXEC via an EXEC mutex so transactions do not interleave
- [x] 1.2 Make the WATCH version check and command apply one critical section (close the TOCTOU window)
- [x] 1.3 WAL batch API for EXEC (moved to phase6k with M-010)
- [x] 1.4 Route every transaction command through WAL/persistence and replication (moved to phase6k, M-010)
- [x] 1.5 Update transaction.rs docs to match the real guarantees
- [x] 1.6 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/transactions.md)
- [x] 2.2 Write tests covering the new behavior (existing MULTI/EXEC/WATCH suite passes; deterministic concurrency tests avoided as flaky)
- [x] 2.3 Run tests and confirm they pass
