# Proposal: phase5_v1-wire-rust-sdk-to-protocol

Source: docs/analysis/synap-v1-release/ (F-005, F-006)

## Why
The Rust SDK (`sdks/rust`, 21 modules) re-implements the wire types that the server defines
in `protocol/synap_rpc/types.rs` (`SynapValue`, `Request`, `Response`) and hand-rolls
MessagePack encode/decode with `rmp-serde`. Every server-side wire change must currently be
mirrored by hand in the SDK — a silent-drift risk the Vectorizer split explicitly called out
("SDK type duplication") and fixed by having the SDK consume the protocol crate. With
`synap-protocol` extracted in phase 3, the SDK can now share one source of truth. The SDK
also hardcodes `version`/`edition` instead of inheriting workspace fields.

## What Changes
1. Per-type diff of SDK wire types vs `synap-protocol` types; where shapes match, delete the
   SDK copy and import from `synap-protocol`. Where shapes intentionally differ (client-side
   ergonomics), keep the SDK type and document the mapping — no silent divergence.
2. `sdks/rust/Cargo.toml`: add `synap-protocol` (path + version dependency so crates.io
   publishing still works), switch `version`/`edition` to `.workspace = true`.
3. SDK transports (SynapRPC/RESP3/HTTP) use `synap-protocol` codecs where applicable instead
   of local `rmp_serde` calls.
4. Full SDK test suite + S2S smoke tests confirm wire compatibility is unchanged.

Gate: `cargo check --workspace` → `clippy -D warnings` → `cargo test` (workspace + SDK).

## Impact
- Affected specs: none (wire format unchanged — that is the point)
- Affected code: `sdks/rust/src/**` (type imports, transports), `sdks/rust/Cargo.toml`
- Breaking change: POSSIBLY for SDK consumers if re-exported type paths change — mitigate by
  re-exporting `synap-protocol` types from the SDK's existing module paths
- User benefit: one source of truth for the wire format; server protocol changes propagate
  to the SDK at compile time instead of drifting silently
