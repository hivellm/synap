## 1. Implementation
- [x] 1.1 Change StoredValue to hold an Arc<[u8]> payload (both variants; constructors convert)
- [x] 1.2 Return the shared buffer from GET via get_shared without an intermediate to_vec()
- [x] 1.3 Store-level read is zero-copy (get_shared); protocol value-enum wiring (RESP3 BulkString->Arc, 89 sites; HTTP/JSON copies) is phase11_wire-value-zero-copy
- [x] 1.4 Bench large-value reads (get vs get_shared) + ptr-equality test confirms the copy is eliminated

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (CHANGELOG + StoredValue docs)
- [x] 2.2 Write tests covering the new behavior (get_shared shares one buffer; set_data COW; APPEND/SETRANGE)
- [x] 2.3 Run tests and confirm they pass
