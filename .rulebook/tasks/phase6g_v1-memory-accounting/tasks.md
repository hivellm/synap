## 1. Implementation
- [x] 1.1 Per-datatype size estimators: memory_bytes() on hash/list/set/sorted_set/stream/queue (commit 1153686)
- [x] 1.2 Shared GlobalMemory budget registry; all stores register their counter, wired in main.rs + 500ms refresh task (commits a689ad3, 6ae7cba)
- [x] 1.3 Eviction/refusal consults the full accounted total: KV eviction on the shared total, hash/list/set grow-writes refuse over cap (commits a689ad3, 1153686)
- [x] 1.4 Shared-buffer reads (Arc<[u8]>) split out — invasive read-path change tracked in phase9_kv-shared-buffer-reads
- [x] 1.5 Per-datatype memory metric synap_datatype_memory_bytes on /metrics scrape (commit 561d237)
- [x] 1.6 Gate: cargo check, clippy -D warnings, fmt --check (green each commit)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/memory-accounting.md + CHANGELOG M-018)
- [x] 2.2 Write tests covering the new behavior (hash counts toward + respects budget; cross-datatype: KV usage refuses a hash write)
- [x] 2.3 Run tests and confirm they pass (full workspace suite: 1714 passed, 0 failed)
