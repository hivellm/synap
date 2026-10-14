## 1. Implementation
- [x] 1.1 Bump the Cargo workspace, TypeScript, Python, C# and PHP SDKs and the GUI to 1.4.0 — workspace.package (all five workspace crates, Rust SDK included, inherit it; internal path deps carry no version requirement), Cargo.lock via `cargo update -w --offline`, TypeScript and GUI package.json/package-lock.json, Python pyproject + `__version__`, C# csproj, README badge and Docker publish examples; the PHP SDK is the `hivellm/synap-sdk-php` submodule with no version field (Packagist versions it from that repo's tags), so nothing to bump here; `cargo check --workspace --locked` and full-tests clippy green; remaining `1.3.2` mentions are historical
- [ ] 1.2 Write the 1.4.0 CHANGELOG entry
- [ ] 1.3 Write the 1.4.0 migration guide: authorization per transport, WAL and snapshot versions, no-downgrade rule and backup step, Hub listener policy, rate limiting, CLI `PING` exit status, read-only GUI configuration
- [ ] 1.4 Run `cargo test-full` and the SDK E2E and S2S suites against the release build
- [ ] 1.5 Start 1.4.0 on a data directory written by 1.3.2 and verify full recovery
- [ ] 1.6 Prepare `release/v1.4.0`; push and tag only with the user's authorization

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
