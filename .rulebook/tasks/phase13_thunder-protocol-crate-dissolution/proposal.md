# Proposal: phase13_thunder-protocol-crate-dissolution

Source: https://github.com/hivellm/thunder — `docs/analysis/05-protocol-crate-dissolution.md`.

> **Sequencing correction.** This task runs **after** phase14, despite the
> numbering. Deleting `crates/synap-protocol/src/synap_rpc/` requires the Rust
> SDK to have stopped importing it, which is phase14's job. The real dependency
> is 14 → 13.

## Why

`crates/synap-protocol` exists for exactly one reason: the Rust SDK needs the wire
types, and crates.io rejects path-only dependencies, so publishing the SDK forces
publishing the protocol crate first. That choreography — bump protocol, publish,
bump the SDK's pin, publish — runs on every wire-touching release.

Worse, the crate is not even pure wire. It carries `envelope.rs` (the HTTP
envelope) and `resp3/` (880 LOC of parser + writer), meaning Synap publishes
server-internal parsing code to a public registry just to hand its SDK ~600 lines
of shared types.

Once phase12 puts the server on Thunder and phase14 puts the Rust SDK on
`thunder-rpc`, the crate has no consumer left. Thunder's dissolution recipe (§5.4)
is: relocate the non-wire residue into the server, ship one terminal deprecated
shim release, then remove the crate from the workspace.

## What Changes

- `crates/synap-protocol/src/resp3/` → `crates/synap-server/src/protocol/resp3/`
  (joins the existing `resp3/server.rs` and `resp3/command/`), internal, never published.
- `crates/synap-protocol/src/envelope.rs` → `crates/synap-server/src/server/envelope.rs`.
- `crates/synap-protocol/src/synap_rpc/` is deleted — Thunder replaces it.
- **`crates/synap-protocol` is deleted outright — no deprecation shim.**
  Thunder's recipe (§5.4) suggests shipping a terminal release of `#[deprecated]`
  re-exports, and one was drafted here before being dropped on the owner's call:
  the crate should simply stop existing, absorbed into the server and the SDK.
  A shim would be a fourth thing to build, version and reason about, in exchange
  for saving hypothetical external consumers one find-and-replace.
  `synap-protocol` 1.0.0 stays on crates.io — crates.io never deletes — and it
  is entirely self-contained, so anyone pinned to it keeps building indefinitely.
  The migration table lives in the CHANGELOG instead of in a published artifact.
- With the crate gone, `cargo publish --dry-run` on the Rust SDK proves zero path
  dependencies and zero product-protocol packages (amended Gate G2).

## Impact
- Affected specs: `.rulebook/tasks/phase13_thunder-protocol-crate-dissolution/specs/workspace-layout/spec.md`
- Affected code: `crates/synap-protocol/`, `crates/synap-server/src/protocol/resp3/`, `crates/synap-server/src/server/`, `Cargo.toml`
- Breaking change: YES for external Rust consumers of `synap-protocol`. They are
  not stranded: the published 1.0.0 is self-contained and keeps building, and the
  CHANGELOG carries the type-by-type migration table to `thunder-rpc`.
- User benefit: Synap releases lose their protocol-publish step permanently; the
  RESP3 parser and HTTP envelope stop being public API.
