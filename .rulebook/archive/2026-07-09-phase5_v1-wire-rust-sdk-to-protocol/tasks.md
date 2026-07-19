## 1. Implementation
- [x] 1.1 Per-type diff: WireValue==SynapValue, RpcRequest==Request, RpcResponse==Response (identical shapes/serde); only SDK-side helpers as_float/is_null/to_json differed — folded into SynapValue. Domain DTOs in types.rs intentionally kept (documented)
- [x] 1.2 Add synap-protocol path+version dependency to sdks/rust/Cargo.toml (version/edition already inherit workspace from phase1)
- [x] 1.3 Replace WireValue/RpcRequest/RpcResponse with synap-protocol imports (WireValue = SynapValue alias); wire types are crate-internal so no public re-export needed
- [x] 1.4 Switch SDK SynapRPC write path to synap-protocol codec::encode_frame; reads decode the shared Response type
- [x] 1.5 Document intentional divergences (client-ergonomic domain DTOs) in SDK README
- [x] 1.6 Gate: cargo check --workspace + clippy -D warnings + fmt all green; SDK unit/transport tests green; live e2e (spawns release server, exercises HTTP+SynapRPC+RESP3 with cross-transport consistency) — 8 passed, 0 failed

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (SDK README "Wire types" section)
- [x] 2.2 Write tests covering the new behavior (synap-protocol accessor/to_json tests + SDK transport round-trip against a mock server)
- [x] 2.3 Run tests and confirm they pass (workspace clippy green; SDK tests green; live e2e 8/8 across all three transports)
