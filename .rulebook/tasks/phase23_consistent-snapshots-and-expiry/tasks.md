## 1. Implementation
- [x] 1.1 Write design.md choosing how a snapshot's state image is paired with its WAL position and how same-key WAL order follows memory order
- [x] 1.2 Implement the snapshot barrier so every operation is in the snapshot image or after its WAL cutoff, never both
- [x] 1.3 Make concurrent writes to the same key reach the WAL in memory order
- [ ] 1.4 Store absolute expiry in snapshot format v4 and in WAL records that set or change expiry, still reading v3 snapshots and legacy records
- [ ] 1.5 Derive absolute expiry for legacy WAL records from the entry timestamp and omit keys already expired at recovery
- [ ] 1.6 Carry absolute expiry through replication
- [ ] 1.7 Test a snapshot paused after its cutoff around LPUSH and HINCRBY, multi-shard data, transactions, concurrent APPEND on one key, and snapshot-only, WAL-only and snapshot+WAL restarts before and after expiry with a controlled clock

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
