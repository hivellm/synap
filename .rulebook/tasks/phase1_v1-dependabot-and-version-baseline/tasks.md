## 1. Implementation
- [ ] 1.1 Batch-merge TS-SDK dev-dep PRs #225, #226, #227, #228, #229 after one combined CI pass
- [ ] 1.2 Merge PR #222 (sysinfo 0.33→0.39) with cargo check + clippy -D warnings + cargo test gate
- [ ] 1.3 Merge PR #223 (mlua 0.11.5→0.12.0) with the same gate; fix scripting/ API churn if any
- [ ] 1.4 Merge PR #224 (rmcp 1.5.0→2.1.0) with the same gate; fix MCP layer API churn
- [ ] 1.5 Prune superseded dependabot branches on origin
- [ ] 1.6 Set [workspace.package] version = "1.0.0-rc.1" in root Cargo.toml
- [ ] 1.7 Convert synap-server/Cargo.toml and sdks/rust/Cargo.toml to version/edition workspace inheritance
- [ ] 1.8 Create release/v1.0.0 branch from updated main (git branch only, no checkout)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
