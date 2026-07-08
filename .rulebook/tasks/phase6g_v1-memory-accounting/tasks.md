## 1. Implementation
- [ ] 1.1 Add per-datatype size estimators (hash/list/set/sorted_set/stream/queue) updated on mutation
- [ ] 1.2 Feed those sizes into the shared maxmemory budget alongside KV
- [ ] 1.3 Make eviction/refusal consider the full accounted total, not KV only
- [ ] 1.4 Store values behind a shared buffer type (Arc<[u8]>/Bytes) so GET returns without a full clone
- [ ] 1.5 Expose accurate per-datatype memory in INFO/metrics
- [ ] 1.6 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (memory/eviction doc)
- [ ] 2.2 Write tests covering the new behavior (collections count toward maxmemory; eviction triggers under collection pressure; GET returns shared buffer)
- [ ] 2.3 Run tests and confirm they pass
