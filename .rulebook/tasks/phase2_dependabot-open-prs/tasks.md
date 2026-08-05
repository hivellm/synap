## 1. Rust dependencies

- [ ] 1.1 Bump `base64` 0.22 → 0.23 in the workspace and the Rust SDK, adapting call sites
- [ ] 1.2 Bump `rmcp` 2.1.0 → 3.0.1 in `synap-server`, adapting the MCP server call sites
- [ ] 1.3 `cargo check --workspace --all-features` + `cargo clippy -- -D warnings` clean

## 2. TypeScript SDK dependencies

- [ ] 2.1 Bump `msgpackr` → ^2.0.5, `@types/node` → ^26.1.2, `eslint` → ^10.8.0
- [ ] 2.2 Type-check, lint and run the TypeScript SDK test suite

## 3. Tail (docs + tests)

- [ ] 3.1 Update or create documentation covering the implementation
- [ ] 3.2 Write tests covering the new behavior
- [ ] 3.3 Run tests and confirm they pass
