## 1. Implementation
- [ ] 1.1 Define inter-node quota RPC request/response messages
- [ ] 1.2 Implement query_master_quota (follower → master quota snapshot)
- [ ] 1.3 Implement send_quota_delta (follower → master consumption delta) + master aggregation
- [ ] 1.4 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior (delta aggregation + query)
- [ ] 2.3 Run tests and confirm they pass
