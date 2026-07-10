## 1. Implementation
- [ ] 1.1 Change StoredValue to hold an Arc<[u8]>/bytes::Bytes payload
- [ ] 1.2 Return the shared buffer from GET/MGET without an intermediate to_vec()
- [ ] 1.3 Adapt HTTP/RESP3/SynapRPC serializers to write from the shared buffer
- [ ] 1.4 Bench large-value reads before/after; confirm the copy is eliminated

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
