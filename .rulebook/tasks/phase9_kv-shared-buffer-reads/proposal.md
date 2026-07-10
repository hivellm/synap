# Proposal: phase9_kv-shared-buffer-reads

Source: docs/analysis/synap-audit/ (M-018, read-copy half — split from phase6g)

## Why
phase6g fixed the cross-datatype memory accounting half of M-018. The other half
— avoiding a full value copy on every read — was intentionally split out because
it is a large, invasive change orthogonal to accounting: every GET does
`value.data().to_vec()` (kv_store/store.rs), so a large-value read allocates a
full copy, doubling memory traffic where Redis returns a shared reference.

## What Changes
1. Store KV values behind a shared buffer type (`Arc<[u8]>` or `bytes::Bytes`) in
   `StoredValue` so a clone is a refcount bump, not a payload copy.
2. Return the shared buffer from the read path (GET/MGET) up to the response
   boundary without an intermediate `to_vec()`; adapt the HTTP/RESP3/SynapRPC
   serializers to write from the shared buffer.
3. Keep the wire format unchanged (bytes on the wire are identical); measure the
   large-value read improvement with a bench.

## Impact
- Affected specs: none (wire format unchanged)
- Affected code: crates/synap-core/src/core/kv_store/store.rs, core/types.rs
  (StoredValue), read/response paths in crates/synap-server/src/
- Breaking change: POSSIBLY for Rust API consumers of internal value types
- User benefit: large-value reads stop allocating a full copy per read
