## 1. Implementation

- [x] 1.1 Pick the replacement crate: maintained, serde-compatible, no unmaintained advisory in its own graph
- [x] 1.2 Prove every file under `config/` parses to the same value with both crates, and pin that each loads as `ServerConfig` in a test
- [x] 1.3 Swap the dependency in the workspace, `synap-server` and `synap-migrate`
- [x] 1.4 Port the call sites in `config.rs` and `config_tests.rs`
- [x] 1.5 Remove RUSTSEC-2024-0449 from the `deny.toml` ignore list

## 2. Tail (docs + tests — check or waive with tailWaiver)

- [x] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [x] 2.3 Run tests and confirm they pass
