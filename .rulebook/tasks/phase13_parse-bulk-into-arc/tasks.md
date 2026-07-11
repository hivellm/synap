## 1. Implementation
- [ ] 1.1 Parser: allocate bulk-string payloads directly as Arc<[u8]> (no Vec intermediate)
- [ ] 1.2 Thread the Arc through Resp3Value args to KVStore::set without re-copy (keep Vec APIs via From)
- [ ] 1.3 SynapRPC: verify/align the Bytes deserialization path (no intermediate copy, or document why)
- [ ] 1.4 Re-run -P 16 sweep; record before/after in docs/benchmarks/redis-vs-synap.md

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
