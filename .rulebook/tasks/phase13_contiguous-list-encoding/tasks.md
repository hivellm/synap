## 1. Implementation
- [ ] 1.1 Contiguous small-list encoding (length-prefixed entries in one buffer) with size thresholds
- [ ] 1.2 Automatic upgrade to VecDeque representation past thresholds; all ops handle both encodings
- [ ] 1.3 Persistence: snapshot round-trip serializes the logical sequence for both encodings
- [ ] 1.4 Re-run sweep: RPUSH/LPOP/RPOP target >= 0.9 of Redis; record in benchmark doc

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
