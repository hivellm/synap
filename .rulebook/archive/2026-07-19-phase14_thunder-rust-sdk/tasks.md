## 1. Dependency swap

- [x] 1.1 Replace the `synap-protocol` dependency in `sdks/rust/Cargo.toml` with `thunder-rpc` (`default-features = false, features = ["client"]`)
- [x] 1.2 Re-point `WireValue` to `thunder::Value` and move the Synap-only value helpers into an SDK-local extension trait (`transport/value_ext.rs` — only `to_json` was needed; numeric coercion already lived at the mapper's call sites)
- [x] 1.3 `cargo check -p synap-sdk` clean

## 2. Transport rewrite

- [x] 2.1 Replace `SynapRpcTransport`'s stream/mutex/id-counter internals with a `thunder::Client` built from the Synap `Config`
- [x] 2.2 Route credentials through `ClientConfig` (`user_pass` / `token`) — the pre-Thunder transport never sent `AUTH` at all, so this is new capability, not a port
- [x] 2.3 Map `thunder::ClientError` onto `SynapError`, adding the `Unauthorized` variant for `NOAUTH`/`WRONGPASS`/`NOPERM` and marking the enum `#[non_exhaustive]`
- [x] 2.4 Delete the superseded framing, reconnect and id-allocation code

## 3. Push path

- [x] 3.1 Consume SUBSCRIBE push frames through `Client::on_push`, returning a `PushSubscription` that owns the connection so the subscription's lifetime is the connection's
- [x] 3.2 Verify a subscriber still receives published messages end-to-end

## 4. Publishability

- [x] 4.1 Confirm `cargo publish --dry-run -p synap-sdk` resolves with zero path dependencies

## 5. Tail (docs + tests — check or waive with tailWaiver)

- [x] 5.1 Update or create documentation covering the implementation — `sdks/rust/README.md` (transport section rewritten) and `CHANGELOG.md` (Unreleased → Changed/Added/Fixed)
- [x] 5.2 Write tests covering the new behavior — `sdks/rust/src/transport/tests.rs` updated for Thunder's accessor semantics plus a mapper-coercion test, and `crates/synap-server/tests/sdk_rpc_e2e_tests.rs` added: 5 tests driving the real SDK against the real server, covering the KV round-trip, 32-way pipelining, a non-ASCII payload, RPC authentication and the typed `Unauthorized` error
- [x] 5.3 Run tests and confirm they pass — `cargo clippy --workspace --all-targets` clean and the full `cargo test --workspace` suite green (91 suites)
