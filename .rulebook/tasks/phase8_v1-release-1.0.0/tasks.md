## 1. Implementation
- [ ] 1.1 Set [workspace.package] version = "1.0.0"; confirm all member crates inherit
- [ ] 1.2 Write CHANGELOG 1.0.0 entry with crate-split migration guide and bind-default note
- [ ] 1.3 Docs sweep: README, AGENTS.override.md workspace tree + layers, DOCKER_README, docs/ architecture pages, helm appVersion
- [ ] 1.4 Delete stray "tatus --short" file at repo root (authorized by this task's approval)
- [ ] 1.5 Full release gate: cargo check, clippy -D warnings, fmt --check, full tests, benches compile, Docker build
- [ ] 1.6 Tag v1.0.0 and open release PR from release/v1.0.0 to main

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
