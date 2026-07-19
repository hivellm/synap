# Proposal: phase3_v1-extract-synap-protocol

Source: docs/analysis/synap-v1-release/ (F-003; supports F-005)

## Why
Following the Vectorizer/Nexus convention, wire-format code belongs in a dedicated
`<name>-protocol` crate so SDKs and tools can parse/serialize without linking the server.
Synap's `protocol/` module is 8,289 LOC across 19 files, but it is NOT a pure wire module:
the RESP3 command layer (`protocol/resp3/command/`, 1,157-LOC dispatcher) and the SynapRPC
dispatch layer (`protocol/synap_rpc/dispatch/`) import `AppState`, `crate::core::*` and
`crate::scripting`. Moving `protocol/` wholesale would create a `protocol → server`
dependency cycle. The extraction must therefore split wire from dispatch — this is the
single riskiest boundary of the whole restructure and deserves its own phase.

## What Changes
1. Create `crates/synap-protocol` containing only pure wire code:
   `protocol/envelope.rs`, `protocol/resp3/parser.rs`, `protocol/resp3/writer.rs`,
   the RESP value type, `protocol/synap_rpc/codec.rs`, `protocol/synap_rpc/types.rs`
   (`SynapValue`, `Request`, `Response`), and their unit tests.
2. Keep `protocol/resp3/command/` and `protocol/synap_rpc/dispatch/` inside `synap-server`
   (rehomed under `server/` or a `dispatch/` module) — they are request handlers, not wire code.
3. `synap-server` depends on `synap-protocol`; imports rewritten
   (`crate::protocol::X` → `synap_protocol::X`) for the moved items only.
4. Guardrail: `synap-protocol` must have NO dependency on `synap-server` or on core stores —
   verified by `cargo check` (a cycle fails to compile) and by reviewing its `Cargo.toml`
   dependency list.

Gate: `cargo check --workspace` → `clippy -D warnings` → `cargo test` (including RESP3
parser/writer and SynapRPC codec unit tests, which move with the code).

## Impact
- Affected specs: none (no wire-format behavior change; code location only)
- Affected code: `crates/synap-server/src/protocol/**` (split), new `crates/synap-protocol/`,
  import sites across `server/`, `main.rs` listener wiring
- Breaking change: NO at runtime; YES for Rust consumers importing `synap_server::protocol::*`
  (migration note lands in phase 8 CHANGELOG)
- User benefit: reusable wire crate (SDK de-duplication in phase 5), faster incremental builds,
  clear wire-vs-dispatch boundary
