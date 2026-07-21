## 1. Implementation
- [x] 1.1 Rename the two wire command names in the C# SDK to the server's dispatch names: `kv.delete` → `kv.del` (KVStore.cs + CommandMapper.cs) and `hash.values` → `hash.vals` (HashManager.cs + CommandMapper.cs), updating the TransportTests mapper cases
- [x] 1.2 Fix the C# HTTP read path: ExecuteHttpAsync unwraps the command envelope payload (objects/arrays flat, scalars as {"value": ...}) so modules parse real data instead of silently returning defaults
- [x] 1.3 Fix the C# native-transport mapper: live keys `hash.del`/`hash.incrby` (payload key `increment`), `hash.del` response shaping, and AsRawArray accepting both `object?[]` (RESP3) and `List<object?>` (SynapRPC) list shapes
- [x] 1.4 Server: accept singular `field` (all SDKs) as well as `fields` in the `hash.del` command; add HINCRBY/HINCRBYFLOAT to the RESP3 and SynapRPC dispatchers; add the stream family (SCREATE/SGETORCREATE/SPUBLISH/SREAD/SDELETE/SLIST/SSTATS) to RESP3, with SCREATE taking the SDKs' optional max_events
- [x] 1.5 Python SDK: queue publish/consume encode/decode the byte-list wire payload (JSON round-trip, symmetric with TS); QCONSUME native replies (map/array) normalized to the HTTP shape; `hash.incrby` native mapping reads `increment`; retry_count read correctly
- [x] 1.6 Python SDK: refuse native mapping for transactional writes (payload with client_id) so they raise UnsupportedCommandError instead of silently executing outside the MULTI; parity tests updated to the explicit contract; stale S2S tests (dataclass stats access, pfadd/pfmerge varargs) fixed
- [x] 1.7 Re-run the manual smoke harness (C#: HTTP/RPC/RESP3 kv+hash) and the full S2S matrix against a live release server — all consistent

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation (CHANGELOG "Fixed" — cross-SDK/server compat; "Added" — HINCRBY/HINCRBYFLOAT + RESP3 stream family)
- [x] 2.2 Write tests covering the new behavior (RESP3 + RPC dispatch tests for HINCRBY/HINCRBYFLOAT; command-endpoint test for hash.del field/fields; C# mapper tests updated; Python parity tests encode the explicit native-transaction contract)
- [x] 2.3 Run tests and confirm they pass (Rust workspace suites green; TS 488 S2S green; Python 231 green incl. S2S; C# 107 green + 3-transport smoke consistent)
