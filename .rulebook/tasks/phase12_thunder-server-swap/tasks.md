## 1. Dependency and configuration

- [ ] 1.1 Add `thunder-rpc = { version = "0.1", default-features = false, features = ["server"] }` to the workspace and `crates/synap-server`
- [ ] 1.2 Add `crates/synap-server/src/protocol/synap_rpc/config.rs` declaring the Synap `thunder::Config` (scheme `synap`, port 15501, `AuthCommand`, `HelloStyle::NotUsed`, `PushPolicy::Enabled`, `Resp3Prefixes`, 512 MiB cap)
- [ ] 1.3 Unit-test the config constant against the values Synap's SDKs assume

## 2. Value-type migration

- [ ] 2.1 Introduce `pub type SynapValue = thunder::Value;` in `crates/synap-server/src/protocol/synap_rpc/mod.rs` and re-point the dispatch tree's imports
- [ ] 2.2 Port `SynapValue`'s Synap-only helpers (`to_json`, `as_float` string parsing, `From` impls) to free functions / an extension trait over `thunder::Value`
- [ ] 2.3 `cargo check -p synap-server` clean

## 3. Dispatch trait

- [ ] 3.1 Implement `thunder::server::Dispatch` for a `SynapDispatch { state: AppState }` newtype, delegating to the existing `dispatch::run`
- [ ] 3.2 Implement `authenticate` over `UserManager` (`AUTH <pass>` → user `default`, `AUTH <user> <pass>`), returning `WRONGPASS` on failure
- [ ] 3.3 Move the per-command admin ACL into `dispatch`, resolving the user from `Session::principal()`
- [ ] 3.4 Record the `synap_rpc_*` Prometheus metrics inside `dispatch`

## 4. Listener and push

- [ ] 4.1 Replace `spawn_synap_rpc_listener`'s accept loop with `thunder::server::spawn_listener` + `ListenerConfig` (idle timeout, `auth_required` from `state.require_auth`)
- [ ] 4.2 Re-implement SUBSCRIBE push registration on `Session::push_sender()`
- [ ] 4.3 Wire the returned `ListenerHandle` into the server's shutdown path
- [ ] 4.4 Delete the superseded writer task, AUTH inline branch and auth gate

## 5. Upstream gaps

- [ ] 5.1 File a `hivellm/thunder` issue for every Synap capability Thunder cannot express (max-connections refusal, frame-size metric hook, principal payload) — one issue per gap, with the Synap call site
- [ ] 5.2 Record the chosen in-repo mitigation for each filed gap in `.rulebook/knowledge/`

## 6. Tail (docs + tests — check or waive with tailWaiver)

- [ ] 6.1 Update or create documentation covering the implementation — `docs/` and `CHANGELOG.md` (Unreleased → Changed) for the Thunder swap and the `Bytes` canonicalization
- [ ] 6.2 Write tests covering the new behavior — port the SynapRPC server tests and add coverage for the three behavior deltas (bin `Bytes`, pre-auth `PING`, `HELLO`)
- [ ] 6.3 Run tests and confirm they pass — `cargo clippy -- -D warnings` plus the full `cargo test` suite
