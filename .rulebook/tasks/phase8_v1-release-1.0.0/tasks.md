## 1. Implementation
- [x] 1.1 Set [workspace.package] version = "1.0.0"; confirm all member crates inherit
- [x] 1.2 Write CHANGELOG 1.0.0 entry with crate-split migration guide and bind-default note
- [x] 1.3 Docs sweep: README, AGENTS.override.md workspace tree + layers, DOCKER_README, docs/ architecture pages, helm appVersion
- [x] 1.4 Delete stray "tatus --short" file at repo root (already absent)
- [x] 1.5 Full release gate: cargo check, clippy -D warnings, fmt --check, full tests, benches compile all green (Docker image builds in CI/Linux; local Docker Desktop hits a rustup cross-device-link quirk during musl toolchain install, unrelated to code)
- [ ] 1.6 Tag v1.0.0 and open release PR from release/v1.0.0 to main

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
