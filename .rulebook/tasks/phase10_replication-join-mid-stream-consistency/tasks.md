## 1. Implementation
- [ ] 1.1 Register the replica in the broadcast set before taking the snapshot
- [ ] 1.2 After full sync, flush replication-log ops with offset > snapshot offset before the live loop
- [ ] 1.3 Ensure replica apply is idempotent so log-flush/live-stream overlap is harmless
- [ ] 1.4 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (replication.md join semantics)
- [ ] 2.2 Write tests covering the new behavior (replica joins mid-write, converges to full dataset)
- [ ] 2.3 Run tests and confirm they pass
