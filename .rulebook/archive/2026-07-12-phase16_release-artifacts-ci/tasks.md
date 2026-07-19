## 1. Implementation
- [x] 1.1 Rewrite `.github/workflows/release.yml`: `release: published` + `workflow_dispatch` tag input (RELEASE_TAG env), `taiki-e/upload-rust-binary-action@v1` for synap-server + synap-cli with `checksum: sha256`, targets linux x64 (ubuntu-latest) / linux arm64 (ubuntu-24.04-arm native) / macOS x64+arm64 / windows x64, `CARGO_INCREMENTAL=0`, independent jobs
- [x] 1.2 Validate workflow YAML parses and job/action inputs match the taiki-e action contract (yaml.safe_load + structural walk in the test script)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/RELEASE_PROCESS.md flow + asset tables + secrets section, CHANGELOG [1.0.0])
- [x] 2.2 Write tests covering the new behavior (scripts/test-release-workflow.py: 13 structural assertions — triggers, 5 targets, both bins, sha256, tag-pinned ref, native arm runner, no cross/vendored leftovers)
- [x] 2.3 Run tests and confirm they pass (13/13 + docker-publish smoke still green)
