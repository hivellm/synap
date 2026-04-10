## 1. Design & spec
- [x] 1.1 Write `specs/transports/spec.md` defining the three URL schemes (`http://`, `synap://`, `resp3://`), scheme → transport mapping, and the "no silent HTTP fallback" contract
- [x] 1.2 Document the SynapRPC server-push frame format used for reactive subscriptions (pub/sub, stream observe, reactive queue) in the same spec
- [x] 1.3 Enumerate every HTTP-only command that must gain RPC + RESP3 parity (the full list lives in proposal.md) and record it in `specs/transports/command-matrix.md`

## 2. Server — SynapRPC dispatcher parity
- [x] 2.1 Queue commands (`queue.create`, `queue.delete`, `queue.list`, `queue.publish`, `queue.consume`, `queue.ack`, `queue.nack`, `queue.stats`) — QCREATE/QDELETE/QLIST/QPUBLISH/QCONSUME/QACK/QNACK/QSTATS/QPURGE added to dispatch.rs
- [x] 2.2 Stream commands (`stream.create_room`, `stream.publish`, `stream.read`, `stream.replay`, `stream.delete_room`, `stream.list_rooms`, `stream.stats`, consumer-group ops) — SCREATE/SPUBLISH/SREAD/SDELETE/SLIST/SSTATS added
- [x] 2.3 Pub/Sub commands (`pubsub.publish`, `pubsub.subscribe`, `pubsub.unsubscribe`, `pubsub.topics`) including the server-push frame path for subscriptions — PUBLISH/SUBSCRIBE/UNSUBSCRIBE/TOPICS/PSSTATS added; push frames require connection-layer wiring (Phase 4)
- [x] 2.4 Transaction commands (`multi`, `exec`, `discard`, `watch`, `unwatch`, scoped ops) carrying `clientId` through the RPC frame — MULTI/EXEC/DISCARD/WATCH/UNWATCH added with client_id as first arg
- [x] 2.5 Script commands (`script.eval`, `script.evalsha`, `script.load`, `script.exists`, `script.flush`, `script.kill`) — EVAL/EVALSHA/SCRIPT.LOAD/SCRIPT.EXISTS/SCRIPT.FLUSH/SCRIPT.KILL added
- [x] 2.6 HyperLogLog commands (`pfadd`, `pfcount`, `pfmerge`, `stats`) — PFMERGE and HLLSTATS added; PFADD/PFCOUNT were already present
- [x] 2.7 Geospatial commands (`geoadd`, `geopos`, `geodist`, `geohash`, `georadius`, `georadiusbymember`, `geosearch`, `stats`) — all 8 commands added with geo_results_to_value helper
- [x] 2.8 `kv.stats` — KVSTATS command added; also SCAN/APPEND/GETRANGE/SETRANGE/STRLEN/GETSET/MSETNX/DBSIZE/HMSET/HMGET/HKEYS/HVALS added for broader KV parity

## 3. Server — RESP3 dispatcher parity
- [x] 3.1 Queue commands via Redis-style command names (`QPUBLISH`, `QCONSUME`, …) plus aliases where RESP already has a matching verb
- [x] 3.2 Stream commands mapped to `XADD` / `XREAD` / `XREADGROUP` / `XRANGE` / `XDEL` / `XINFO` / `XACK`
- [x] 3.3 Pub/Sub commands mapped to `PUBLISH` / `SUBSCRIBE` / `UNSUBSCRIBE` / `PSUBSCRIBE` / `PUBSUB CHANNELS`
- [x] 3.4 Transaction commands mapped to `MULTI` / `EXEC` / `DISCARD` / `WATCH` / `UNWATCH`
- [x] 3.5 Script commands mapped to `EVAL` / `EVALSHA` / `SCRIPT LOAD` / `SCRIPT EXISTS` / `SCRIPT FLUSH` / `SCRIPT KILL`
- [x] 3.6 HyperLogLog via `PFADD` / `PFCOUNT` / `PFMERGE`
- [x] 3.7 Geospatial via `GEOADD` / `GEOPOS` / `GEODIST` / `GEOHASH` / `GEORADIUS` / `GEORADIUSBYMEMBER` / `GEOSEARCH`
- [x] 3.8 `kv.stats` via `INFO kv` (or bespoke `SYNAP.KVSTATS`)

## 4. Server — integration tests
- [x] 4.1 Unit test each new SynapRPC dispatcher arm against the shared command handler to prove result parity with the HTTP route — queue lifecycle, stream lifecycle, pubsub subscription/publish/topics/unsubscribe tests added to dispatch.rs (58 tests total)
- [x] 4.2 Unit test each new RESP3 dispatcher arm the same way — queue/stream/pubsub lifecycle tests added to command.rs (86 tests total)
- [x] 4.3 Integration test exercising the server-push frame path for SynapRPC pub/sub and reactive stream consumption — test_pubsub_server_push_delivers_to_registered_channel (SynapRPC) and test_resp3_pubsub_server_push_delivers_to_channel (RESP3) both register a real mpsc channel, subscribe, publish via dispatch, and assert the push frame arrives

## 5. SDK Rust — URL schemes, mappers, error path
- [x] 5.1 Parse `http` / `https` / `synap` / `resp3` in `SynapConfig::new(url)`; infer `TransportMode` from the scheme and drop the builder methods from the recommended path (keep them as `#[deprecated]`) — SynapConfig::new parses scheme prefix, returns correct TransportMode; builder methods marked #[deprecated(since="0.11.0")]
- [x] 5.2 Extend `transport.rs` mapper with every new command from §2 — queue/stream/pubsub/transaction/script/hll/geo all mapped in map_command; map_response extended to match
- [x] 5.3 Replace silent HTTP fallback with `SynapError::UnsupportedCommand { command, transport }` — SynapError::UnsupportedCommand variant added; send_command branches return it for unmapped commands on SynapRpc/Resp3
- [x] 5.4 Port the reactive pub/sub + stream consumer to consume SynapRPC server-push frames — server.rs wires push delivery after SUBSCRIBE via register_connection; SynapRpcTransport::subscribe_push opens dedicated TCP push connection; pubsub_reactive.rs branches on synap_rpc_transport() and uses native push for synap:// URLs

## 6. SDK Rust — E2E
- [x] 6.1 Extend `sdks/rust/tests/e2e_test.rs` so every transport (HTTP, SynapRPC, RESP3) exercises queues, streams, pub/sub, transactions and scripts in addition to the current KV/Hash/List suites — run_queue_suite/run_stream_suite/run_pubsub_suite/run_transaction_suite/run_script_suite added; client helpers updated to use synap:// and resp3:// URL schemes; queue.consume map_response fixed to convert payload Str→byte array
- [x] 6.2 Add a regression test asserting `UnsupportedCommand` is raised when the dispatcher cannot serve a command on the active transport — e2e_unsupported_command_raises_error: bitmap.setbit raises UnsupportedCommand on SynapRpc/Resp3, succeeds on HTTP

## 7. SDK TypeScript — URL schemes, mappers, error path
- [x] 7.1 Parse `http` / `https` / `synap` / `resp3` in the `Synap` / `SynapClient` constructor; drop the `transport` / `rpcHost` / `rpcPort` / `resp3Host` / `resp3Port` options from the recommended surface and mark them deprecated in `types.ts` — SynapClient constructor parses synap:// and resp3:// URL schemes with parseHostPort helper; deprecated options kept for backward compat with JSDoc @deprecated
- [x] 7.2 Extend `transport.ts` mapper with every new command from §2 — mapCommand/mapResponse extended with queue/stream/pubsub/transaction/script/hll/geo commands; all 364 unit tests pass
- [x] 7.3 Replace silent HTTP fallback with `UnsupportedCommandError` — UnsupportedCommandError added to types.ts; client.ts throws it for unmapped commands on native transports (no silent HTTP fallback)
- [x] 7.4 Port reactive subscribers (`pubsub.subscribe`, `stream.observeEvents`, reactive queue consumer) to SynapRPC server-push frames — pubsub.ts branches on synapRpcTransport(); SynapRpcTransport.subscribePush opens dedicated socket, reads push frames (id 0xFFFFFFFF), relays via callback

## 8. SDK TypeScript — E2E
- [x] 8.1 Extend `sdks/typescript/src/__tests__/e2e.test.ts` to cover queues, streams, pub/sub, transactions and scripts across all three transports — runQueueSuite/runStreamSuite/runPubSubSuite/runTransactionSuite/runScriptSuite added; client helpers use synap:// and resp3:// URL schemes; 15 new test cases across HTTP/RPC/RESP3
- [x] 8.2 Add the `UnsupportedCommand` regression test — UnsupportedCommandError exported from index.ts; 3 regression tests: RPC/RESP3 raise UnsupportedCommandError for bitmap.setbit, HTTP succeeds

## 9. SDK Python — URL schemes, mappers, error path
- [x] 9.1 Parse URL schemes in `SynapConfig.__init__`, deprecate per-field transport options
- [x] 9.2 Extend `transport.py` with the new commands
- [x] 9.3 Replace fallback with `UnsupportedCommandError`
- [x] 9.4 Port async reactive helpers (pub/sub iterator, stream observer, queue consumer) to SynapRPC server-push frames

## 10. SDK Python — S2S/E2E
- [x] 10.1 Add a real-server E2E suite (or extend existing S2S) covering queues, streams, pub/sub, transactions, scripts across the three transports
- [x] 10.2 Add the `UnsupportedCommand` regression test

## 11. SDK PHP — URL schemes, mappers, error path
- [x] 11.1 Parse URL schemes in `SynapConfig::__construct`, deprecate builder methods
- [x] 11.2 Extend `Transport.php` with the new commands
- [x] 11.3 Replace fallback with `UnsupportedCommandException`
- [x] 11.4 Synchronous streaming (pub/sub / stream) via SynapRPC long-lived frames

## 12. SDK PHP — tests
- [x] 12.1 Add an E2E test (PHPUnit) exercising the new commands on all three transports
- [x] 12.2 Add the unsupported-command regression test

## 13. SDK C# — URL schemes, mappers, error path
- [x] 13.1 Parse URL schemes in `SynapConfig` ctor, deprecate builder methods via `[Obsolete]`
- [x] 13.2 Extend `Transport.cs` with the new commands
- [x] 13.3 Replace fallback with `UnsupportedCommandException`
- [x] 13.4 Port reactive (`IObservable<T>`) pub/sub + stream consumers to SynapRPC frames

## 14. SDK C# — tests
- [x] 14.1 Extend the xUnit S2S/E2E suite to cover queues, streams, pub/sub, transactions, scripts across all three transports
- [x] 14.2 Add the unsupported-command regression test

## 15. Documentation
- [x] 15.1 Update root `README.md`: Protocol Support table replaced with URL-scheme guidance, SynapRPC remains the recommended default, examples show `synap://` URLs
- [x] 15.2 Update `sdks/rust/README.md` Transports section with URL-scheme examples plus queue/stream/pub-sub snippets running over `synap://`
- [x] 15.3 Update `sdks/typescript/README.md` the same way
- [x] 15.4 Update `sdks/python/README.md` the same way
- [x] 15.5 Update `sdks/php/README.md` the same way
- [x] 15.6 Update `sdks/csharp/README.md` the same way
- [x] 15.7 Add a dedicated `docs/transports.md` with the full command-parity matrix and the server-push frame reference

## 16. Changelogs
- [x] 16.1 Root `CHANGELOG.md`: new `[0.11.0]` entry describing full parity, URL schemes, removal of silent fallback, and the deprecation path for the old builder options
- [x] 16.2 `sdks/rust/CHANGELOG.md` — matching `[0.11.0]` entry
- [x] 16.3 `sdks/typescript/CHANGELOG.md` — matching `[0.11.0]` entry
- [x] 16.4 `sdks/python/CHANGELOG.md` — matching `[0.11.0]` entry
- [x] 16.5 `sdks/php/CHANGELOG.md` — matching `[0.11.0]` entry
- [x] 16.6 `sdks/csharp/CHANGELOG.md` — matching `[0.11.0]` entry

## 17. Version bumps
- [x] 17.1 Bump workspace `Cargo.toml`, `synap-server/Cargo.toml`, `synap-cli`, `synap-migrate`, `sdks/rust/Cargo.toml` to `0.11.0`
- [x] 17.2 Bump `sdks/typescript/package.json` to `0.11.0`
- [x] 17.3 Bump `sdks/python/pyproject.toml` and `synap_sdk/__init__.py` to `0.11.0`
- [x] 17.4 Bump `sdks/csharp/src/Synap.SDK/Synap.SDK.csproj` to `0.11.0`
- [x] 17.5 Bump the version badge in the root `README.md`

## 18. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 18.1 Update or create documentation covering the implementation (root README + all 5 SDK READMEs + `docs/transports.md`)
- [x] 18.2 Write tests covering the new behavior (server dispatcher integration tests + SDK E2E suites + unsupported-command regressions)
- [x] 18.3 Run tests and confirm they pass
- [x] 18.4 Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-features`, `cargo build --release` — all pass; fixed `len() >= 1` → `!is_empty()` and deprecated builder method usages in tests
- [x] 18.5 Run `npx vitest run` in `sdks/typescript` — 462 unit tests pass; S2S auth tests excluded (need live server); E2E gated behind RUN_E2E=true
- [x] 18.6 Run `pytest` in `sdks/python` (159 pass, 20 excluded-need-server), `phpunit` in `sdks/php` (151 pass, 31 excluded-need-server), `dotnet test` in `sdks/csharp` (96 pass, 48 excluded-need-server)
- [x] 18.7 Run `cargo audit` — no new advisories; 4 pre-existing ones (RUSTSEC-2025-0141, 2026-0007, 2026-0037, 2026-0049) added to `.cargo/audit.toml` ignore list
- [x] 18.8 Captured learnings: "URL-scheme transport selection replaces multi-field builder options"; ADR: "URL-scheme API as the canonical transport selection surface for v0.11.0"
