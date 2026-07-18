## 1. Dependency swap

- [ ] 1.1 Replace the `synap-protocol` dependency in `sdks/rust/Cargo.toml` with `thunder-rpc` (`default-features = false, features = ["client"]`)
- [ ] 1.2 Re-point `WireValue` to `thunder::Value` and move the Synap-only value helpers into an SDK-local extension trait
- [ ] 1.3 `cargo check -p synap-sdk` clean

## 2. Transport rewrite

- [ ] 2.1 Replace `SynapRpcTransport`'s stream/mutex/id-counter internals with a `thunder::Client` built from the Synap `Config`
- [ ] 2.2 Route credentials through `ClientConfig` (`user_pass` / `api_key`) and drop the hand-rolled AUTH frame
- [ ] 2.3 Map `thunder::ClientError` onto `SynapError`, preserving today's error variants for auth failures and timeouts
- [ ] 2.4 Delete the superseded framing, reconnect and id-allocation code

## 3. Push path

- [ ] 3.1 Consume SUBSCRIBE push frames through `Client::on_push` in the reactive pub/sub module
- [ ] 3.2 Verify a subscriber still receives published messages end-to-end

## 4. Publishability

- [ ] 4.1 Confirm `cargo publish --dry-run -p synap-sdk` resolves with zero path dependencies

## 5. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 5.1 Update or create documentation covering the implementation — `sdks/rust/README.md` and `CHANGELOG.md` (Unreleased → Changed)
- [ ] 5.2 Write tests covering the new behavior — keep `sdks/rust/src/transport/tests.rs` green and add a pipelining test proving concurrent in-flight requests on one connection
- [ ] 5.3 Run tests and confirm they pass — `cargo clippy -- -D warnings` plus `cargo test -p synap-sdk`
