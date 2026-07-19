## 1. Implementation
- [x] 1.1 Scaffold crates/synap-core (Cargo.toml with workspace inheritance, engine deps, lib.rs)
- [x] 1.2 Move core/ (incl. error.rs / SynapError) into synap-core with its unit tests
- [x] 1.3 Move cache/, compression/, simd/, cluster/ into synap-core
- [x] 1.4 Re-export via alias so crate::core::X and crate::cluster::X resolve unchanged
- [x] 1.5 Compile AppState + dispatch layers against synap-core + synap-protocol
- [x] 1.6 Verify DAG: synap-core has no dependency on synap-server/synap-protocol (cargo tree confirms leaf)
- [x] 1.7 Point applicable bench targets at synap-core via synap_server re-exports
- [x] 1.8 Gate: cargo check --workspace, clippy -D warnings, fmt --check, cargo test all green

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
