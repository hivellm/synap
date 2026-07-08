## 1. Implementation
- [ ] 1.1 git mv synap-server, synap-cli, synap-migrate into crates/ (no source changes)
- [ ] 1.2 Root Cargo.toml: members = ["crates/*", "sdks/rust"] + full workspace.package/dependencies/lints inheritance (Nexus pattern)
- [ ] 1.3 Update .github/ CI workflow paths referencing old crate locations
- [ ] 1.4 Update Dockerfile, docker-compose.yml, and helm/ paths
- [ ] 1.5 Update scripts/ and benchmark invocations referencing old paths
- [ ] 1.6 Gate: cargo check --workspace, clippy -D warnings, fmt --check, cargo test all green

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (README, AGENTS.override.md workspace tree, DOCKER_README)
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
