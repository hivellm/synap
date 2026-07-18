## 1. Relocate the non-wire residue

- [x] 1.1 Move `crates/synap-protocol/src/resp3/{parser.rs,writer.rs}` into `crates/synap-server/src/protocol/resp3/` and re-point imports
- [x] 1.2 Move `crates/synap-protocol/src/envelope.rs` into `crates/synap-server/src/server/envelope.rs` and re-point imports
- [x] 1.3 `cargo check --workspace` clean

## 2. Retire the wire module

- [x] 2.1 Delete `crates/synap-protocol/src/synap_rpc/` (superseded by `thunder::wire` in phase12)
- [x] 2.2 Confirm no crate in the workspace still depends on `synap-protocol` — `synap_bench` and `protocol_bench` re-pointed to `thunder`

## 3. Delete the crate

- [x] 3.1 Delete `crates/synap-protocol` outright — no deprecation shim (owner's call; see the proposal for the reasoning and what happens to external consumers)
- [x] 3.2 Put the type-by-type migration table in `CHANGELOG.md`, since there is no published artifact to carry it

## 4. Verify the workspace

- [x] 4.1 Confirm `crates/synap-protocol` is gone from the workspace and no manifest references it
- [x] 4.2 Verify `cargo build --release` and the server binary still build

## 5. Tail (docs + tests — check or waive with tailWaiver)

- [x] 5.1 Update or create documentation covering the implementation — `CHANGELOG.md` (Removed + migration table) and the workspace-structure section of `AGENTS.override.md`
- [x] 5.2 Write tests covering the new behavior — the relocated RESP3 parser/writer and envelope keep their unit tests in their new home, and the full suite proves the relocation changed no behavior
- [x] 5.3 Run tests and confirm they pass — `cargo clippy --workspace --all-targets` clean and the full `cargo test --workspace` suite green (89 suites)
