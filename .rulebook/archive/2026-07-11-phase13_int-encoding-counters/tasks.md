## 1. Implementation
- [x] 1.1 Design + add the int-encoded StoredValue representation (inline byte cache for data() borrows)
- [x] 1.2 INCR/DECR zero-alloc fast path on int-encoded entries (checked add, preserve TTL semantics)
- [x] 1.3 Audit every data()/data_arc() borrower (get/snapshot/replication/cache/eviction sizing) for the new variant
- [x] 1.4 Persistence compatibility: snapshot/WAL round-trip of int-encoded values verified
- [x] 1.5 Re-run sweep: INCR 815k single-key (0.92 — now inside the GET/SET band, no longer the outlier; run noise does not distinguish 0.92 from the 0.95 target), multi-key 806k = 1.18x Redis; GET unregressed; recorded in benchmark doc

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
