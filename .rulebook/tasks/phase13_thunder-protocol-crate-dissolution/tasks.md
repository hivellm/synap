## 1. Relocate the non-wire residue

- [ ] 1.1 Move `crates/synap-protocol/src/resp3/{parser.rs,writer.rs,mod.rs}` into `crates/synap-server/src/protocol/resp3/` and re-point imports
- [ ] 1.2 Move `crates/synap-protocol/src/envelope.rs` into `crates/synap-server/src/server/envelope.rs` and re-point imports
- [ ] 1.3 `cargo check --workspace` clean

## 2. Retire the wire module

- [ ] 2.1 Delete `crates/synap-protocol/src/synap_rpc/` (superseded by `thunder::wire` in phase12)
- [ ] 2.2 Confirm no crate in the workspace still depends on `synap-protocol`

## 3. Terminal shim

- [ ] 3.1 Rewrite `crates/synap-protocol/src/lib.rs` as `#[deprecated]` re-exports of `thunder::wire` with the old type names aliased (`pub type SynapValue = thunder::Value;`)
- [ ] 3.2 Add the deprecation notice to `crates/synap-protocol/README.md` pointing at `thunder::wire`
- [ ] 3.3 Set the shim version to 1.1.0 and verify `cargo publish --dry-run -p synap-protocol`

## 4. Remove from the workspace

- [ ] 4.1 Drop `crates/synap-protocol` from the workspace members once the shim is cut
- [ ] 4.2 Verify `cargo build --release` and the server binary still build

## 5. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 5.1 Update or create documentation covering the implementation — `CHANGELOG.md` (Unreleased → Removed/Deprecated) and the workspace-structure section of `AGENTS.override.md`
- [ ] 5.2 Write tests covering the new behavior — keep the relocated RESP3 and envelope unit tests green in their new home and assert the shim's aliases compile
- [ ] 5.3 Run tests and confirm they pass — `cargo clippy -- -D warnings` plus the full `cargo test` suite
