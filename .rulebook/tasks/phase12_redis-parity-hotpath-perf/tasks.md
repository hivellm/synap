## 1. Implementation
- [x] 1.1 Baseline sweep: capture -P 16 Synap-vs-Redis ratios for all ops
- [x] 1.2 Dispatch: uppercase command into a stack buffer (no per-command String alloc) — RESP3 + SynapRPC
- [x] 1.3 INCR/DECR: parse i64 from bytes + in-place set_data (no read to_vec, no key re-insert)
- [ ] 1.4 Collection stats to atomics (SetStore/ListStore/HashStore/SortedSetStore) — remove per-op stats RwLock
- [ ] 1.5 Arg parsing: reduce per-argument String/Vec allocation on the hot GET/SET path
- [ ] 1.6 SADD (and peers): single get_mut fast-path, drop the get-then-entry double lookup + key alloc
- [ ] 1.7 Re-sweep after each batch; keep changes that move the numbers, document architectural gaps

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
