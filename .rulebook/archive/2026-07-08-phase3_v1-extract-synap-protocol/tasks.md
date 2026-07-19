## 1. Implementation
- [x] 1.1 Scaffold crates/synap-protocol (Cargo.toml with workspace inheritance, lib.rs)
- [x] 1.2 Move envelope.rs + RESP value type into synap-protocol
- [x] 1.3 Move resp3/parser.rs and resp3/writer.rs (with their unit tests) into synap-protocol
- [x] 1.4 Move synap_rpc/codec.rs and synap_rpc/types.rs (with their unit tests) into synap-protocol
- [x] 1.5 Rehome resp3/command/ and synap_rpc/dispatch/ inside synap-server (dispatch stays server-side)
- [x] 1.6 Rewrite imports crate::protocol::X to synap_protocol::X; synap-server depends on synap-protocol
- [x] 1.7 Verify no cycle: synap-protocol has no synap-server/core deps (cargo tree confirms leaf)
- [x] 1.8 Gate: clippy --all-targets -D warnings, fmt --check, cargo test all green

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
