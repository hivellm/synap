## 1. Interop matrix

- [ ] 1.1 Add `scripts/thunder-interop/` driving one Thunder-based server build against every SDK
- [ ] 1.2 Cover per SDK: authenticate → SET/GET with a binary value → SUBSCRIBE/PUBLISH → error round-trip
- [ ] 1.3 Run the matrix for rust, typescript, python, csharp, go
- [ ] 1.4 Run the matrix for php and java (hand-written transports, no Thunder package)
- [ ] 1.5 Run the legacy cell: a pre-Thunder SDK build against the new server, proving int-array `Bytes` tolerance
- [ ] 1.6 Record the full matrix result in `docs/`

## 2. Failures

- [ ] 2.1 For each red cell caused by Thunder, file an issue on `hivellm/thunder` with the failing frame bytes and the SDK/language
- [ ] 2.2 For each red cell caused by Synap, fix it in this repo and re-run the affected cell
- [ ] 2.3 Confirm every cell is green before continuing to section 3

## 3. Release

- [ ] 3.1 Bump the workspace and every SDK manifest to 1.1.0
- [ ] 3.2 Write the `CHANGELOG.md` 1.1.0 section with the `Bytes` canonicalization, the `synap-protocol` deprecation and a migration note
- [ ] 3.3 Update `README.md` and `docs/` to describe the RPC transport as Thunder-based
- [ ] 3.4 Publish the terminal `synap-protocol` shim prepared in phase13
- [ ] 3.5 Run the full quality gate: `cargo clippy -- -D warnings`, `cargo test`, and every SDK's test suite
- [ ] 3.6 Tag `v1.1.0` and verify the release artifacts build

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 4.1 Update or create documentation covering the implementation — the interop matrix, its results and how to re-run it, in `docs/`
- [ ] 4.2 Write tests covering the new behavior — wire the interop matrix into CI as a regression gate
- [ ] 4.3 Run tests and confirm they pass — the complete verification once more on the release commit
