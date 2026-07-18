## 1. Dependency and configuration

- [x] 1.1 Add `thunder-rpc = { version = "0.2", default-features = false, features = ["server"] }` to the workspace and `crates/synap-server`
- [x] 1.2 Add `crates/synap-server/src/protocol/synap_rpc/config.rs` declaring the Synap `thunder::Config` (scheme `synap`, port 15501, `AuthCommand`, `HelloStyle::NotUsed`, `PushPolicy::Enabled`, `Resp3Prefixes`, 512 MiB cap)
- [x] 1.3 Unit-test the config constant against the values Synap's SDKs assume

## 2. Value-type migration

- [x] 2.1 Introduce `pub type SynapValue = thunder::Value;` in `crates/synap-server/src/protocol/synap_rpc/mod.rs` and re-point the dispatch tree's imports
- [x] 2.2 Port `SynapValue`'s Synap-only helpers (`to_json`, `as_float` string parsing, `From` impls) to free functions / an extension trait over `thunder::Value` — not needed: `synap-server` used none of them (they are SDK-side, handled in phase14)
- [x] 2.3 `cargo check -p synap-server` clean

## 3. Dispatch trait

- [x] 3.1 Implement `thunder::server::Dispatch` for a `SynapDispatch { state: AppState }` newtype, delegating to the existing `dispatch::run`
- [x] 3.2 Implement `authenticate` over `UserManager` (`AUTH <pass>` → user `default`, `AUTH <user> <pass>`), returning `WRONGPASS` on failure
- [x] 3.3 Move the per-command admin ACL into `dispatch`, resolving the user from the session — carried as `Dispatch::Identity = User`, so it costs one store lookup per connection
- [x] 3.4 Record the `synap_rpc_*` Prometheus metrics — via Thunder's `MetricsObserver`, which fires where the listener records its own counters

## 4. Listener and push

- [x] 4.1 Replace `spawn_synap_rpc_listener`'s accept loop with `thunder::server::spawn_listener` + `ListenerConfig` (idle timeout, `max_connections`, `auth_required` from `state.require_auth`)
- [x] 4.2 Re-implement SUBSCRIBE push registration on `Session::push_sender()`
- [x] 4.3 Wire the returned `ListenerHandle` into the server's shutdown path
- [x] 4.4 Delete the superseded writer task, AUTH inline branch and auth gate

## 5. Upstream gaps

- [x] 5.1 File a `hivellm/thunder` issue for every Synap capability Thunder cannot express — five filed (thunder#1 zero-copy `Bytes`, #2 connection ceiling, #3 metrics hook, #4 session identity, #5 shareable handle); all five fixed upstream in 0.1.2/0.2.0 and every Synap mitigation reverted
- [x] 5.2 Record the chosen in-repo mitigation for each filed gap in `.rulebook/knowledge/`

## 6. Tail (docs + tests — check or waive with tailWaiver)

- [x] 6.1 Update or create documentation covering the implementation — `docs/network-limits.md` and `CHANGELOG.md` (Unreleased → Changed) for the Thunder swap and the `Bytes` canonicalization
- [x] 6.2 Write tests covering the new behavior — `crates/synap-server/tests/synap_rpc_thunder_tests.rs`: 16 tests over a real socket covering the catalog, pipelining, auth/ACL, push, the frame cap, the connection ceiling, graceful drain, and the three wire deltas (bin `Bytes`, pre-auth `PING`, legacy int-array tolerance)
- [x] 6.3 Run tests and confirm they pass — `cargo clippy --workspace --all-targets` clean and the full `cargo test --workspace` suite green
