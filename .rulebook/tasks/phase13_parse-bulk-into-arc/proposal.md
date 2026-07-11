# Proposal: phase13_parse-bulk-into-arc

Source: docs/analysis/redis-parity-deep-dive.md (item D, plan round 5)

## Why

Every KV write copies the value twice. The RESP3 parser allocates a `Vec<u8>`
per bulk argument (`parser.rs:137`), then `StoredValue::new` converts it with
`data.into()` — `Vec<u8> → Arc<[u8]>` allocates the `ArcInner` and memcpys the
whole payload again (`types.rs:121`). Redis pays one tight `sdsnewlen` from the
query buffer. This is the largest remaining per-write cost after the phase12
rounds (SET at 0.94 of Redis).

## What Changes

1. The RESP3 parser's bulk-string path allocates the payload directly as a
   shared buffer (`Arc::new_uninit_slice(len)` + `read_exact` into it, or an
   equivalent safe construction) so `BulkString`/argument bytes are born as
   `Arc<[u8]>`.
2. `Resp3Value` bulk arguments carry the `Arc`; `arg_bytes`/`cmd_set` hand it to
   the store without re-copying; `StoredValue` accepts the `Arc` as-is.
3. SynapRPC equivalent where practical (`SynapValue::Bytes` is already
   `Arc<[u8]>` — ensure rmp-serde deserialization lands in the Arc without an
   intermediate copy, or document why not).
4. Re-run the -P 16 sweep; expected: SET/APPEND/LPUSH-family writes gain a few
   percent (one full memcpy per write removed).

## Impact

- Affected specs: none (wire format unchanged; internal representation only)
- Affected code: crates/synap-protocol/src/resp3/parser.rs,
  crates/synap-server/src/protocol/resp3/command/*.rs,
  crates/synap-core/src/core/types.rs (constructors), kv_store/store.rs
- Breaking change: NO (public `set(Vec<u8>)` API kept via `From` impls)
- User benefit: lower write latency/CPU per SET — one allocation + memcpy
  removed from every value write
