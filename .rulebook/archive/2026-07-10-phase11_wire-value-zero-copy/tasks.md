## 1. Implementation
- [x] 1.1 Change Resp3Value::BulkString (+ SynapValue byte variant) to Arc<[u8]>; update construction sites/parser/writer
- [x] 1.2 Wire RESP3/SynapRPC GET/MGET handlers to get_shared without an intermediate to_vec()
- [x] 1.3 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior (RESP3 GET returns bytes unchanged; no copy)
- [x] 2.3 Run tests and confirm they pass
