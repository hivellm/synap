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
- A terminal `synap-protocol` 1.1.0 shim is prepared: `#[deprecated]` re-exports
  pointing at `thunder::wire`, README notice. Prepared and versioned in this task;
  published in phase19.
- `crates/synap-protocol` is removed from the workspace members after the shim is
  cut, so `cargo publish --dry-run` on the Rust SDK proves zero path dependencies
  and zero product-protocol packages (amended Gate G2).

## Impact
- Affected specs: `.rulebook/tasks/phase13_thunder-protocol-crate-dissolution/specs/workspace-layout/spec.md`
- Affected code: `crates/synap-protocol/`, `crates/synap-server/src/protocol/resp3/`, `crates/synap-server/src/server/`, `Cargo.toml`
- Breaking change: YES for external Rust consumers of `synap-protocol` — mitigated
  by the terminal deprecated shim, which keeps them compiling.
- User benefit: Synap releases lose their protocol-publish step permanently; the
  RESP3 parser and HTTP envelope stop being public API.
