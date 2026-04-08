## 1. SynapRPC Protocol Crate (synap-rpc/)
- [ ] 1.1 Create `synap-rpc/` workspace member with `Cargo.toml`; add to workspace `[members]`
- [ ] 1.2 Implement `synap-rpc/src/types.rs` — `SynapValue` enum (Null, Bool, Int(i64), Float(f64), Bytes(Vec<u8>), Str(String), Array(Vec<SynapValue>), Map(Vec<(SynapValue, SynapValue)>)); `Request { id: u32, command: String, args: Vec<SynapValue> }`; `Response { id: u32, result: Result<SynapValue, String> }`
- [ ] 1.3 Implement `synap-rpc/src/codec.rs` — `encode_frame(msg: &impl Serialize) -> Vec<u8>` (4-byte LE u32 length + rmp_serde MessagePack body); `decode_frame(buf: &[u8]) -> Option<(T, usize)>` — returns parsed value and bytes consumed
- [ ] 1.4 Implement `synap-rpc/src/client.rs` — `SynapRpcClient` with `connect(addr)`, `send(Request) -> Response`, connection pool (min 1, max 8 connections per client), request multiplexing via `DashMap<u32, oneshot::Sender<Response>>`
- [ ] 1.5 Add `rmp-serde`, `serde`, `tokio`, `dashmap` to `synap-rpc/Cargo.toml`

## 2. RESP3 Server (synap-server/src/protocol/resp3/)
- [ ] 2.1 Create `synap-server/src/protocol/resp3/parser.rs` — hand-written RESP3 parser: `+` simple string, `-` error, `:` integer, `$` bulk string, `*` array, `_` null, `,` double, `#` boolean, `=` verbatim, `~` set, `%` map; returns `Resp3Value` enum
- [ ] 2.2 Create `synap-server/src/protocol/resp3/writer.rs` — `Resp3Writer` that serializes `Resp3Value` back to bytes; used for all server responses
- [ ] 2.3 Create `synap-server/src/protocol/resp3/command.rs` — maps RESP3 command arrays to internal handler calls: SET, GET, DEL, INCR, DECR, EXPIRE, TTL, PERSIST, EXISTS, KEYS, SCAN, MSET, MGET, HSET, HGET, HDEL, HGETALL, LPUSH, RPUSH, LRANGE, SADD, SMEMBERS, SREM, ZADD, ZRANGE, ZSCORE, BITCOUNT, BITOP, PFADD, PFCOUNT, PING, AUTH, SELECT, QUIT
- [ ] 2.4 Create `synap-server/src/protocol/resp3/server.rs` — `spawn_resp3_listener(state, addr)`: `TcpListener::bind`, accept loop, per-connection task with read-parse-dispatch-write pipeline; inline pipelining (read N commands before flushing)
- [ ] 2.5 Wire AUTH: if `auth.enabled`, first command on connection MUST be `AUTH password`; reject all others with `-NOAUTH` until authenticated

## 3. SynapRPC Server (synap-server/src/protocol/synap_rpc/)
- [ ] 3.1 Create `synap-server/src/protocol/synap_rpc/server.rs` — `spawn_synap_rpc_listener(state, addr)`: accept loop, per-connection task
- [ ] 3.2 Per-connection task: read 4-byte length header, read body, decode `Request` via `synap_rpc::codec::decode_frame`, dispatch to internal handler, encode `Response`, write back — supports concurrent in-flight requests on one connection via `tokio::spawn` per request
- [ ] 3.3 Create `synap-server/src/protocol/synap_rpc/dispatch.rs` — maps `Request.command` string to handler function; reuses same handler logic as HTTP path (no duplication)
- [ ] 3.4 Auth: first frame on connection must be `{ command: "AUTH", args: [password] }`; reject others until authenticated when auth enabled

## 4. Server Integration (synap-server/src/main.rs + config)
- [ ] 4.1 Add `resp3: Resp3Config { enabled: bool, port: u16 }` and `synap_rpc: SynapRpcConfig { enabled: bool, port: u16 }` to `ServerConfig`; defaults `enabled: false`, ports `6379` and `15501`
- [ ] 4.2 In `main.rs`: if `resp3.enabled`, `tokio::spawn(spawn_resp3_listener(state.clone(), addr))`; same for synap_rpc
- [ ] 4.3 Log startup message for each enabled protocol listener

## 5. Rust SDK Update (sdks/rust/)
- [ ] 5.1 Add `synap-rpc` as optional dependency under `[features] tcp = ["synap-rpc"]`
- [ ] 5.2 Add `Transport` enum to `SynapConfig`: `Http`, `Tcp(addr)`, `Auto(addr)` — `Auto` connects via TCP, falls back to HTTP on connection failure
- [ ] 5.3 Implement `TcpTransport` in `sdks/rust/src/transport/tcp.rs` using `synap_rpc::client::SynapRpcClient`
- [ ] 5.4 `SynapClient::send_command()` dispatches to active transport; all existing KV/hash/list/set methods unchanged

## 6. TypeScript SDK Update (sdks/typescript/)
- [ ] 6.1 Add `src/transport/tcp.ts` — `TcpTransport` using Node.js `net.Socket`; implements the SynapRPC frame codec (4-byte LE + msgpack via `@msgpack/msgpack`)
- [ ] 6.2 Add `transport?: "http" | "tcp" | "auto"` and `tcpPort?: number` to `SynapClientConfig`
- [ ] 6.3 `SynapClient` constructor selects transport; `auto` mode probes TCP on connect, falls back to HTTP
- [ ] 6.4 All existing public methods unchanged — transport is internal

## 7. Python SDK Update (sdks/python/)
- [ ] 7.1 Add `synap/transport/tcp.py` — `TcpTransport` using `asyncio.open_connection`; SynapRPC codec using `msgpack` library
- [ ] 7.2 Add `transport: Literal["http", "tcp", "auto"] = "auto"` param to `SynapClient.__init__`
- [ ] 7.3 All existing async methods unchanged

## 8. C# SDK Update (sdks/csharp/)
- [ ] 8.1 Add `Transport/TcpTransport.cs` — `TcpTransport` using `System.Net.Sockets.TcpClient`; SynapRPC codec using `MessagePack-CSharp` NuGet
- [ ] 8.2 Add `Transport` property to `SynapClientConfig` with enum `Http | Tcp | Auto`
- [ ] 8.3 All existing methods unchanged

## 9. PHP SDK Update (sdks/php/)
- [ ] 9.1 Add `src/Transport/TcpTransport.php` — uses `stream_socket_client`; SynapRPC codec using `msgpack_pack`/`msgpack_unpack` (ext-msgpack)
- [ ] 9.2 Add `transport` config option (`'http'|'tcp'|'auto'`) to `SynapClient`
- [ ] 9.3 All existing methods unchanged

## 10. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 10.1 Write RESP3 parser unit tests: cover all type prefixes, multi-bulk arrays, inline commands, partial reads (split across two TCP reads)
- [ ] 10.2 Write SynapRPC codec round-trip tests: encode then decode for every SynapValue variant including nested arrays and maps
- [ ] 10.3 Write integration test: connect via RESP3 on port 6379, run SET/GET/DEL/PING, assert correct responses (requires server running under `#[cfg(feature = "s2s")]`)
- [ ] 10.4 Write integration test: connect via SynapRPC on port 15501, run 1000 concurrent requests on one connection, assert all responses arrive with correct request_id
- [ ] 10.5 Benchmark: `cargo bench --bench redis_vs_synap --features s2s` comparing HTTP vs SynapRPC vs RESP3 on SET/GET — assert SynapRPC p50 latency < HTTP p50 / 3
- [ ] 10.6 Run `cargo test --workspace --all-features` — all tests pass
- [ ] 10.7 Update or create documentation covering the implementation
- [ ] 10.8 Write tests covering the new behavior
- [ ] 10.9 Run tests and confirm they pass
