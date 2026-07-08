## 1. Implementation
- [ ] 1.1 Scaffold crates/synap-protocol (Cargo.toml with workspace inheritance, lib.rs)
- [ ] 1.2 Move envelope.rs + RESP value type into synap-protocol
- [ ] 1.3 Move resp3/parser.rs and resp3/writer.rs (with their unit tests) into synap-protocol
- [ ] 1.4 Move synap_rpc/codec.rs and synap_rpc/types.rs (with their unit tests) into synap-protocol
- [ ] 1.5 Rehome resp3/command/ and synap_rpc/dispatch/ inside synap-server (dispatch stays server-side)
- [ ] 1.6 Rewrite imports crate::protocol::X → synap_protocol::X for moved items; synap-server depends on synap-protocol
- [ ] 1.7 Verify no cycle: synap-protocol Cargo.toml has no synap-server/core deps; cargo check --workspace green
- [ ] 1.8 Gate: clippy -D warnings, fmt --check, cargo test all green

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (crate README + module docs)
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
