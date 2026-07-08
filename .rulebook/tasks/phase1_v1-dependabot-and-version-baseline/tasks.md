## 1. Implementation
- [x] 1.1 Bump TS-SDK dev-deps (typescript-eslint 8.62.1→8.63.0, vitest + coverage-v8 4.1.9→4.1.10) on release/v1.0.0
- [x] 1.2 sysinfo 0.33→0.39 with cargo check + clippy -D warnings + cargo test gate (no API churn)
- [x] 1.3 mlua 0.11.5→0.12.0 with the same gate (no API churn)
- [x] 1.4 rmcp 1.5.0→2.1.0 with the same gate; MCP layer fix (model::Content → model::ContentBlock)
- [ ] 1.5 Close PRs #222-#229 and prune superseded dependabot branches on origin (remote sync, needs push access)
- [x] 1.6 Set [workspace.package] version = "1.0.0-rc.1" in root Cargo.toml
- [x] 1.7 Convert synap-server/Cargo.toml and sdks/rust/Cargo.toml to version/edition workspace inheritance
- [x] 1.8 Create release/v1.0.0 branch from main

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass

> Note: dependency updates were applied as commits on release/v1.0.0 (commit 9cfc073)
> rather than merging the dependabot PRs into main, to keep main stable while the
> breaking rmcp 2.1 bump was adapted. Closing/superseding the GitHub PRs is item 1.5.
