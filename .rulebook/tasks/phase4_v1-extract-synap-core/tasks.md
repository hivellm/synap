## 1. Implementation
- [ ] 1.1 Scaffold crates/synap-core (Cargo.toml with workspace inheritance, engine-only deps, lib.rs)
- [ ] 1.2 Move core/ (23 files incl. error.rs) into synap-core with its unit tests
- [ ] 1.3 Move cache/, compression/, simd/ into synap-core
- [ ] 1.4 Rewrite imports crate::core::X → synap_core::X module by module, one commit per module, cargo check after each
- [ ] 1.5 Compile AppState + dispatch layers against synap-core + synap-protocol
- [ ] 1.6 Verify DAG: synap-core has no dependency on synap-server/synap-protocol
- [ ] 1.7 Point applicable bench targets at synap-core
- [ ] 1.8 Gate: cargo check --workspace, clippy -D warnings, fmt --check, cargo test all green

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (crate README + AGENTS.override.md layers)
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
