> **Status: shipped in 0.10.0** (commits `57353f9` multi-transport SDKs, `bc85bfd` v0.10.0 release).
>
> **Architectural deviation from the original plan:** the standalone
> `synap-rpc/` workspace crate was NOT created. The SynapRPC types,
> codec, server and dispatcher live directly inside
> `synap-server/src/protocol/synap_rpc/` (`codec.rs`, `types.rs`,
> `server.rs`, `dispatch.rs`, `mod.rs`). Each SDK ships a single
> `transport.*` file instead of a `transport/tcp.*` submodule. Full
> RPC + RESP3 command parity and URL-scheme constructors are tracked
> separately in `phase3_full-rpc-resp3-parity-and-url-schemes`.

## 1. SynapRPC Protocol (in-server, not a standalone crate)
- [x] 1.1 Code placed in `synap-server/src/protocol/synap_rpc/` instead of a standalone `synap-rpc/` crate
- [x] 1.2 `SynapValue` / `Request` / `Response` implemented in `synap-server/src/protocol/synap_rpc/types.rs`
- [x] 1.3 Frame codec (4-byte LE length + rmp_serde MessagePack body) in `synap-server/src/protocol/synap_rpc/codec.rs`
- [x] 1.4 Client implemented per-SDK (see §5–§9); no shared Rust client crate
- [x] 1.5 `rmp-serde`, `serde`, `tokio`, `dashmap` present in `synap-server/Cargo.toml`

## 2. RESP3 Server (synap-server/src/protocol/resp3/)
- [x] 2.1 `parser.rs` — hand-written RESP3 parser covering all type prefixes
- [x] 2.2 `writer.rs` — serializes `Resp3Value` to bytes for server responses
- [x] 2.3 `command.rs` — maps RESP3 command arrays to internal handlers (SET/GET/DEL/INCR/EXPIRE/HSET/LPUSH/SADD/ZADD/BITCOUNT/PFADD/PING/AUTH/…)
- [x] 2.4 `server.rs` — `spawn_resp3_listener(state, addr)` with per-connection read-parse-dispatch-write pipeline
- [x] 2.5 AUTH gate honoured when `auth.enabled`

## 3. SynapRPC Server (synap-server/src/protocol/synap_rpc/)
- [x] 3.1 `server.rs` — `spawn_synap_rpc_listener(state, addr)` with accept loop + per-connection task
- [x] 3.2 Per-connection read length + body, decode, dispatch, encode, write back; concurrent in-flight requests via `tokio::spawn`
- [x] 3.3 `dispatch.rs` — command-name → handler routing, reusing the same core logic as the HTTP path
- [x] 3.4 AUTH gate on first frame when auth enabled

## 4. Server Integration (synap-server/src/main.rs + config)
- [x] 4.1 `Resp3Config { enabled, port }` and `SynapRpcConfig { enabled, port }` added to `ServerConfig` (defaults `6379` / `15501`)
- [x] 4.2 `main.rs` spawns each listener when enabled
- [x] 4.3 Startup log line per enabled protocol listener

## 5. Rust SDK Update (sdks/rust/)
- [x] 5.1 `Transport` enum (`Http`, `SynapRpc`, `Resp3`) wired into `SynapConfig`
- [x] 5.2 Builder methods `with_synap_rpc_transport()`, `with_resp3_transport()`, `with_rpc_addr()`, `with_resp3_addr()` (URL-scheme migration tracked in phase3)
- [x] 5.3 SynapRPC + RESP3 clients implemented directly in `sdks/rust/src/transport.rs` (single file)
- [x] 5.4 `SynapClient` dispatches to the active transport; HTTP fallback for unmapped commands (removal tracked in phase3)

## 6. TypeScript SDK Update (sdks/typescript/)
- [x] 6.1 SynapRPC + RESP3 clients implemented in `sdks/typescript/src/transport.ts` using `net.Socket` and `@msgpack/msgpack`
- [x] 6.2 `transport`, `rpcHost`, `rpcPort`, `resp3Host`, `resp3Port` options on `SynapClientConfig`
- [x] 6.3 Transport selection at construction time; HTTP fallback for unmapped commands
- [x] 6.4 Public API unchanged
- [x] 6.5 E2E suite `src/__tests__/e2e.test.ts` exercises HTTP, SynapRPC and RESP3 transports + cross-transport consistency (gated behind `RUN_E2E=true`)

## 7. Python SDK Update (sdks/python/)
- [x] 7.1 SynapRPC + RESP3 clients in `sdks/python/synap_sdk/transport.py` (asyncio + `msgpack`)
- [x] 7.2 `transport` kwarg on `SynapClient.__init__`
- [x] 7.3 Public async API unchanged

## 8. C# SDK Update (sdks/csharp/)
- [x] 8.1 SynapRPC + RESP3 clients in `sdks/csharp/src/Synap.SDK/Transport.cs` using `System.Net.Sockets.TcpClient` and `MessagePack-CSharp`
- [x] 8.2 Transport selection via `SynapConfig` builder
- [x] 8.3 Public API unchanged

## 9. PHP SDK Update (sdks/php/)
- [x] 9.1 SynapRPC + RESP3 clients in `sdks/php/src/Transport.php` using `stream_socket_client` and `rybakit/msgpack`
- [x] 9.2 Transport config option on `SynapClient`
- [x] 9.3 Public API unchanged

## 10. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 10.1 RESP3 parser unit tests (type prefixes, multi-bulk arrays, partial reads)
- [x] 10.2 SynapRPC codec round-trip tests for every `SynapValue` variant
- [x] 10.3 Rust E2E integration test: `sdks/rust/tests/e2e_test.rs` drives HTTP / SynapRPC / RESP3 against a real server
- [x] 10.4 TypeScript E2E integration test: `sdks/typescript/src/__tests__/e2e.test.ts` — exposed and fixed 3 transport bugs (Bytes→string, RESP3 framing, Bool→int coercion)
- [x] 10.5 Cross-transport benchmark `redis_vs_synap` is folded into `phase3_full-rpc-resp3-parity-and-url-schemes` so all three transports cover the same command surface before measurement
- [x] 10.6 `cargo test --workspace --all-features` green; `cargo fmt` + `cargo clippy -- -D warnings` clean
- [x] 10.7 Documentation updated: root `README.md` Protocol Support section, all 5 SDK READMEs' Transports sections, root + 5 SDK `CHANGELOG.md` `[0.10.0]` entries
- [x] 10.8 Write tests covering the new behavior (see 10.1–10.4)
- [x] 10.9 Run tests and confirm they pass
- [x] 10.10 Update or create documentation covering the implementation (root README Protocol Support, all 5 SDK README Transports sections, root + 5 SDK CHANGELOG `[0.10.0]` entries)
