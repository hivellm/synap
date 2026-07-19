## 1. Implementation
- [x] 1.1 git mv synap-server, synap-cli, synap-migrate into crates/ (no source changes)
- [x] 1.2 Root Cargo.toml: members = ["crates/*", "sdks/rust"] with workspace inheritance
- [x] 1.3 Update .github/ CI workflow paths referencing old crate locations
- [x] 1.4 Update Dockerfile paths (docker-compose/helm use context + Dockerfile)
- [x] 1.5 Update scripts/ and synap-cli path-dep referencing old paths
- [x] 1.6 Gate: cargo check --workspace, clippy -D warnings, fmt --check, cargo test all green

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
