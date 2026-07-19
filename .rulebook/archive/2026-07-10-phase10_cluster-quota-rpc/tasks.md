## 1. Implementation
- [x] 1.1 Define inter-node quota RPC request/response messages (length-prefixed bincode)
- [x] 1.2 Implement query_master_quota (follower → master quota snapshot via GetQuota)
- [x] 1.3 Implement send_quota_delta (follower → master via ApplyDeltas) + master aggregation
- [x] 1.4 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (CHANGELOG + module docs)
- [x] 2.2 Write tests covering the new behavior (query + delta roundtrip; no-master fallback)
- [x] 2.3 Run tests and confirm they pass
