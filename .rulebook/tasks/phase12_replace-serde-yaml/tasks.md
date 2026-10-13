## 1. Implementation

- [ ] 1.1 Pick the replacement crate: maintained, serde-compatible, no unmaintained advisory in its own graph
- [ ] 1.2 Capture how every file under `config/` parses today, as a golden test on `ServerConfig`
- [ ] 1.3 Swap the dependency in the workspace, `synap-server` and `synap-migrate`
- [ ] 1.4 Port the call sites in `config.rs` and `config_tests.rs`
- [ ] 1.5 Remove RUSTSEC-2024-0449 from the `deny.toml` ignore list

## 2. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
