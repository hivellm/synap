# Proposal: phase11_wire-value-zero-copy

Source: phase9_kv-shared-buffer-reads follow-up (protocol-boundary half)

## Why
phase9 stored KV values behind a shared `Arc<[u8]>` and added `KVStore::get_shared`
so a read is a refcount bump, not a copy. The store no longer copies on read.
What remains is threading that shared buffer through the protocol value enums to
the socket so the binary path (RESP3/SynapRPC) is zero-copy end-to-end. Today
`Resp3Value::BulkString(Vec<u8>)` (89 construction sites) and `SynapValue` hold
owned `Vec<u8>`, so a GET reply still copies the value into the reply value before
the writer serializes it. (HTTP/JSON is inherently copying and out of scope.)

## What Changes
1. Change `Resp3Value::BulkString` (and the equivalent SynapValue byte variant)
   to hold `Arc<[u8]>` (or `bytes::Bytes`); update the ~89 construction sites
   (mostly `.into()`), the parser, and the writer (which already writes `&[u8]`).
2. Wire the RESP3/SynapRPC GET (and MGET) handlers to `get_shared`, passing the
   `Arc` into the reply value with no intermediate `to_vec()`.
3. Re-run the `large_value_read` benchmark end-to-end over the socket to confirm
   the wire path no longer copies large values.

## Impact
- Affected specs: none (wire bytes identical)
- Affected code: crates/synap-protocol/src/resp3/{parser,writer}.rs, synap_rpc
  value type, crates/synap-server/src/protocol/**, GET/MGET handlers
- Breaking change: NO (wire format unchanged)
- User benefit: large-value reads over the binary protocols avoid a full copy
