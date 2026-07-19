## 1. Interop matrix

- [x] 1.1 Add `scripts/interop/` driving one Thunder-based server build against every SDK
- [x] 1.2 Cover per SDK: authenticate → SET/GET with a binary value → SUBSCRIBE/PUBLISH → error round-trip
- [x] 1.3 Run the matrix for rust, typescript, python, csharp, go
- [x] 1.4 Run the matrix for php (green after three fixes). Java has no toolchain on
      this machine — Maven is absent from the winget source and the JDK 17 install was
      cancelled at the UAC prompt — so its cell is recorded as unverified in
      `docs/thunder-interop-matrix.md` rather than presented as a result
- [x] 1.5 Run the legacy cell: a pre-Thunder SDK build against the new server, proving int-array `Bytes` tolerance
- [x] 1.6 Record the full matrix result in `docs/thunder-interop-matrix.md`

## 2. Failures

- [x] 2.1 No red cell was caused by Thunder — every defect the matrix surfaced was
      Synap's own and predates the swap, so no new upstream issue was filed
- [x] 2.2 Fixed and re-ran: rust (JSON round-trip asymmetry), typescript (lossy UTF-8
      `Bytes` decode), go (missing `AUTH`; non-UTF-8 `str`), php (missing `AUTH`;
      reserved push id on `SUBSCRIBE`; non-UTF-8 `str`)
- [ ] 2.3 Confirm every cell is green before continuing to section 3 — **blocked**:
      `go`/kv_binary is red because `sendRPC` JSON-marshals the payload and Go's
      `encoding/json` destroys non-UTF-8 before framing. The fix is the `sendRPC`
      rewrite phase20 performs; `java` is unverified for lack of a toolchain

## 3. Release

- [ ] 3.1 Bump the workspace and every SDK manifest to 1.1.0
- [ ] 3.2 Write the `CHANGELOG.md` 1.1.0 section with the `Bytes` canonicalization and a migration
      note. The `synap-protocol` part is already written: phase13 put the type-by-type migration
      table in `CHANGELOG.md` precisely because there is no published artifact to carry it —
      fold it into the 1.1.0 section rather than restating it
- [ ] 3.3 Update `README.md` and `docs/` to describe the RPC transport as Thunder-based
- [x] 3.4 ~~Publish the terminal `synap-protocol` shim prepared in phase13~~ — **void, nothing to
      do.** `phase13_thunder-protocol-crate-dissolution` (archived 2026-07-19, after this task was
      written) decided the opposite: *"Delete `crates/synap-protocol` outright — no deprecation
      shim (owner's call)"*. The crate is gone from the workspace and no shim was ever prepared,
      so this step has no artifact to publish. It also matches the current direction across the
      family: no `-protocol` crate gets published to crates.io
- [ ] 3.5 Run the full quality gate: `cargo clippy -- -D warnings`, `cargo test`, and every SDK's test suite
- [ ] 3.6 Tag `v1.1.0` and verify the release artifacts build

## 4. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 4.1 Update or create documentation covering the implementation — the interop matrix, its results and how to re-run it, in `docs/`
- [ ] 4.2 Write tests covering the new behavior — wire the interop matrix into CI as a regression gate
- [ ] 4.3 Run tests and confirm they pass — the complete verification once more on the release commit
