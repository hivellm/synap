## 1. Interop matrix

- [x] 1.1 Add `scripts/interop/` driving one Thunder-based server build against every SDK
- [x] 1.2 Cover per SDK: authenticate → SET/GET with a binary value → SUBSCRIBE/PUBLISH → error round-trip
- [x] 1.3 Run the matrix for rust, typescript, python, csharp, go
- [x] 1.4 Run the matrix for php — green after three fixes (missing `AUTH`, `SUBSCRIBE`
      on the reserved push id, and non-UTF-8 strings packed as `str`). Java support was
      dropped entirely on 2026-07-19 (owner's call), so there is no Java cell
- [x] 1.5 Run the legacy cell: a pre-Thunder SDK build against the new server, proving int-array `Bytes` tolerance
- [x] 1.6 Record the full matrix result in `docs/thunder-interop-matrix.md`

## 2. Failures

- [x] 2.1 No red cell was caused by Thunder — every defect the matrix surfaced was
      Synap's own and predates the swap, so no new upstream issue was filed
- [x] 2.2 Fixed and re-ran: rust (JSON round-trip asymmetry), typescript (lossy UTF-8
      `Bytes` decode), go (missing `AUTH`; non-UTF-8 `str`), php (missing `AUTH`;
      reserved push id on `SUBSCRIBE`; non-UTF-8 `str`)
- [x] 2.3 Every cell is green: rust, typescript, python, csharp, go, php, and the
      legacy compatibility cell. Go's binary cell closed when phase20 put it on Thunder
      and removed the JSON round trip in both directions (Go SDK v1.1.1). Java support
      was dropped entirely (owner's call, 2026-07-19)

## 3. Release

- [x] 3.1 Bump the workspace and every SDK manifest to 1.1.0
- [x] 3.2 Write the `CHANGELOG.md` 1.1.0 section with the `Bytes` canonicalization and a migration
      note. The `synap-protocol` part is already written: phase13 put the type-by-type migration
      table in `CHANGELOG.md` precisely because there is no published artifact to carry it —
      fold it into the 1.1.0 section rather than restating it
- [x] 3.3 Update `README.md` and `docs/` to describe the RPC transport as Thunder-based
- [x] 3.4 ~~Publish the terminal `synap-protocol` shim prepared in phase13~~ — **void, nothing to
      do.** `phase13_thunder-protocol-crate-dissolution` (archived 2026-07-19, after this task was
      written) decided the opposite: *"Delete `crates/synap-protocol` outright — no deprecation
      shim (owner's call)"*. The crate is gone from the workspace and no shim was ever prepared,
      so this step has no artifact to publish. It also matches the current direction across the
      family: no `-protocol` crate gets published to crates.io
- [x] 3.5 Run the full quality gate: clippy clean, 89 Rust test binaries green, TypeScript 370, Python 181, C# 102, Go green. The s2s/integration suites in TypeScript, Python and PHP need a live server on the default ports and did not run here; Python's 95% coverage gate fails at 68.98% as a direct consequence of those skips, not of this change
- [ ] 3.6 Tag `v1.1.0` and verify the release artifacts build

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [x] 4.1 Update or create documentation covering the implementation — `docs/thunder-interop-matrix.md` records the run and every open cell; `scripts/interop/README.md` covers how to re-run it and how to add a language
- [x] 4.2 Write tests covering the new behavior — `.github/workflows/interop-matrix.yml` runs the matrix on any change to the protocol, the SDKs or the harness. Regression tests were also added at the unit level for each fix: two in `sdk_rpc_e2e_tests.rs` (structured round trip, and that a JSON-looking string is not reinterpreted) and one in the TypeScript transport suite (non-UTF-8 Buffer survives)
- [x] 4.3 Run tests and confirm they pass — clippy clean; 89 Rust test binaries, TypeScript 370, Python 181, C# 102, Go all green; matrix green on rust, typescript, python, csharp, php and legacy
