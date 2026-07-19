# Proposal: phase12_thunder-server-swap

Source: https://github.com/hivellm/thunder — `docs/analysis/04-adoption-plan.md` (P2),
`docs/analysis/05-protocol-crate-dissolution.md` (§5.5 step 1).

## Why

Synap originated the binary RPC protocol (SynapRPC) that Nexus specified and
Vectorizer ported. That same ~600-line wire layer now exists in 18 hand-maintained
copies across the HiveLLM family, and byte-level drift has already appeared.
Thunder (`thunder-rpc` 0.1.1 on crates.io) is the single shared implementation:
one wire, one codec, one client contract, conformance-pinned to a golden-vector
corpus in five languages.

Adopting Thunder in the Synap server means the RPC hot path stops being Synap's
to maintain, gains capabilities Synap never had (frame cap enforced on encode as
well as decode, graceful drain, session state machine, `HELLO` support, TLS as an
opt-in layer), and puts Synap on the family release train. It is also the
precondition for dissolving `crates/synap-protocol` (phase13) and for the SDK
swaps (phase14–phase18).

## What Changes

- `crates/synap-server` gains a `thunder-rpc` dependency (`features = ["server"]`,
  `default-features = false`).
- `crates/synap-server/src/protocol/synap_rpc/server.rs` — the hand-rolled accept
  loop, writer task, AUTH inline handling, auth gate and pubsub push registration
  are replaced by a `thunder::server::Dispatch` implementation plus
  `thunder::server::spawn_listener`.
- A Synap `thunder::Config` is declared in-repo (Thunder ships no per-product
  profiles): scheme `synap`, port 15501, `Handshake::AuthCommand`,
  `HelloStyle::NotUsed`, `PushPolicy::Enabled`, `ErrorConvention::Resp3Prefixes`,
  `max_frame_bytes` 512 MiB (matching today's `MAX_FRAME_SIZE`).
- The dispatch tree (`dispatch/{kv,collections,advanced}.rs`, ~1 900 LOC) is
  retained unchanged in behavior; only its value type changes from
  `synap_protocol::synap_rpc::types::SynapValue` to `thunder::Value` via a
  crate-local alias.
- Per-command ACL (`command_requires_admin`) moves into the `Dispatch` impl,
  resolving the admin flag from `Session::principal()`.
- SUBSCRIBE push registration moves onto `Session::push_sender()`.
- Prometheus metrics (`synap_rpc_*`) are recorded inside the `Dispatch` impl
  rather than in the writer task.

### Behavior deltas accepted by this task

1. **`Bytes` canonicalization** — the server begins emitting MessagePack `bin`
   instead of the int-array form. Thunder decodes both forever. This is the one
   externally visible wire change in the whole plan (adoption plan §4 P2).
2. **Pre-auth allowlist** — Thunder permits `PING`/`HELLO`/`AUTH`/`QUIT` before
   authentication; Synap previously refused `PING` with `NOAUTH`.
3. **`HELLO`** — previously unhandled (fell through to the dispatch tree as an
   unknown command); now answered by Thunder's handshake layer.

### Known gaps to raise upstream

Anything Thunder cannot express that Synap requires today (per-listener
`max_connections` refusal, frame-size metric hooks, richer `Principal`) is filed
as an issue on `hivellm/thunder` rather than worked around silently.

## Impact
- Affected specs: `.rulebook/tasks/phase12_thunder-server-swap/specs/synap-rpc-server/spec.md`
- Affected code: `crates/synap-server/src/protocol/synap_rpc/`, `crates/synap-server/Cargo.toml`, `Cargo.toml`
- Breaking change: NO for SDK users (wire-compatible; `Bytes` bin form is already
  decoded by every Synap SDK). YES for Rust code importing
  `synap_protocol::synap_rpc::types::SynapValue` — aliased for source compatibility.
- User benefit: enforced frame caps in both directions, graceful shutdown,
  `HELLO` negotiation, TLS-ready transport, and one shared protocol implementation
  instead of a per-product copy.
