## 1. Implementation
- [ ] 1.1 SADD multi-key: ahash for SetStore key map + SetValue member set; cache/remove per-op SystemTime timestamps; re-measure (target >= 0.9)
- [ ] 1.2 Key lock: try_read_owned fast path (avoid the async acquire when uncontended)
- [ ] 1.3 Investigate c=200 write collapse (parked parking_lot writers / per-request task spawning); apply the smallest fix that restores >= 0.8 of c=50 throughput
- [ ] 1.4 Re-run multi-key sweep c=50 + c=200; record before/after in the benchmark doc

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
