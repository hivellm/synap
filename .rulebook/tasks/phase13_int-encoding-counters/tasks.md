## 1. Implementation
- [ ] 1.1 Design + add the int-encoded StoredValue representation (inline byte cache for data() borrows)
- [ ] 1.2 INCR/DECR zero-alloc fast path on int-encoded entries (checked add, preserve TTL semantics)
- [ ] 1.3 Audit every data()/data_arc() borrower (get/snapshot/replication/cache/eviction sizing) for the new variant
- [ ] 1.4 Persistence compatibility: snapshot/WAL round-trip of int-encoded values verified
- [ ] 1.5 Re-run sweep: INCR target >= 0.95 of Redis, GET unregressed; record in benchmark doc

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
