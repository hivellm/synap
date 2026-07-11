## 1. Implementation
- [x] 1.1 Baseline sweep: capture -P 16 Synap-vs-Redis ratios for all ops
- [x] 1.2 Dispatch: uppercase command into a stack buffer (no per-command String alloc) — RESP3 + SynapRPC
- [x] 1.3 INCR/DECR: parse i64 from bytes + in-place set_data (no read to_vec, no key re-insert)
- [x] 1.4 Collection stats to atomics (ListStore + SetStore) — remove per-op stats RwLock
- [x] 1.5 GET: borrow the key from the frame (no owned String on the read path)
- [x] 1.6 SADD: single get_mut fast-path, drop the get-then-entry double lookup + key alloc
- [x] 1.7 Re-sweep after each batch; keep changes that move the numbers, document architectural gaps

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
