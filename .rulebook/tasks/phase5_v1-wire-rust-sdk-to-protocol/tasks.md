## 1. Implementation
- [ ] 1.1 Produce per-type diff: SDK wire types vs synap-protocol types (document matches/divergences)
- [ ] 1.2 Add synap-protocol dependency to sdks/rust/Cargo.toml; switch version/edition to workspace inheritance
- [ ] 1.3 Replace matching SDK type copies with synap-protocol imports, re-exported from existing SDK paths
- [ ] 1.4 Switch SDK transports to synap-protocol codecs where applicable
- [ ] 1.5 Document intentional divergences for client-ergonomic types kept in the SDK
- [ ] 1.6 Gate: cargo check --workspace, clippy -D warnings, cargo test (workspace + SDK); S2S smoke tests against a running server

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (SDK README wire-types section)
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
