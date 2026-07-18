# Proposal: phase14_thunder-rust-sdk

Source: https://github.com/hivellm/thunder — `docs/analysis/05-protocol-crate-dissolution.md` §5.5 step 2.

## Why

`sdks/rust` currently reaches the SynapRPC listener through a hand-written
`SynapRpcTransport`: one `TcpStream` behind a `Mutex`, a monotonic id counter, and
a reconnect-on-first-error policy. It is serialized — the mutex means one in-flight
request per connection, so the `id` field it dutifully increments buys nothing —
and it has no connect timeout, no per-call timeout, no bounded in-flight, and no
push hook.

Thunder's client is the family's multiplexer: a background reader demultiplexing by
id, real pipelining, 10 s connect / 30 s per-call timeouts, lazy reconnect with
capped retries, frame caps on encode and decode, typed error parsing for
`NOAUTH`/`WRONGPASS`/`[code]`, and a push hook the SDK's pub/sub path can use
instead of a second connection.

It also removes the SDK's path dependency on `synap-protocol`, which is what makes
phase13's dissolution publishable (Thunder amended Gate G2).

## What Changes

- `sdks/rust/Cargo.toml`: `synap-protocol` (path + version) is replaced by
  `thunder-rpc = { version = "0.1", default-features = false, features = ["client"] }`.
- `pub(crate) use synap_protocol::synap_rpc::SynapValue as WireValue` becomes
  `pub type WireValue = thunder::Value;`, keeping every call site source-compatible.
- The Synap-only helpers on the old `SynapValue` (`to_json`, string-parsing
  `as_float`, `From` impls) move into the SDK as an extension trait.
- `SynapRpcTransport`'s socket handling, framing, id allocation and reconnect are
  replaced by `thunder::Client`, configured with the same Synap `Config`
  (`AuthCommand`, `PushPolicy::Enabled`, scheme `synap`, port 15501) the server
  declares in phase12.
- Credentials flow through `ClientConfig::user_pass` / `api_key` instead of a
  hand-rolled AUTH frame.
- The pub/sub reactive path consumes push frames via `Client::on_push`.
- The public SDK API (`SynapClient`, `TransportMode`, the module surface) is
  unchanged.

## Impact
- Affected specs: `.rulebook/tasks/phase14_thunder-rust-sdk/specs/rust-sdk-transport/spec.md`
- Affected code: `sdks/rust/src/transport/`, `sdks/rust/src/pubsub_reactive.rs`, `sdks/rust/Cargo.toml`
- Breaking change: NO for SDK users — the public API is preserved; `WireValue` stays
  a working alias.
- User benefit: concurrent in-flight requests on one connection, enforced timeouts,
  bounded memory on hostile frames, and typed auth errors.
