# Proposal: Binary TCP Protocol — RESP3 + SynapRPC for All SDKs

## Why

Every SDK operation currently goes through HTTP: TCP handshake + HTTP headers (~400 bytes) +
JSON serialization/deserialization on each request. For a high-frequency cache workload this
adds 50-200 microseconds of protocol overhead per operation — overhead that Redis avoids
entirely by using a compact text protocol (RESP) directly over TCP.

Measured: Synap HTTP GET at ~150us round-trip on localhost vs Redis RESP GET at ~30us — 5x
slower not due to KV logic but pure transport overhead. After the KV hot-path optimizations in
Phase 1, the HTTP layer is now the dominant bottleneck.

Two protocols are implemented:

1. RESP3 (Redis Serialization Protocol v3): Text-based, line-delimited, industry standard.
   Allows existing Redis clients (redis-cli, redis-py, ioredis, redis-rs) to connect without
   any code change. Port 6379 (Redis default). This satisfies Phase 2.1 in execution-plan.md.

2. SynapRPC: Synap's native binary protocol over TCP. Length-prefixed frames (4-byte LE u32
   header) with MessagePack payload. Supports multiplexed async requests on a single
   connection (request_id field), streaming responses, and binary values without base64.
   Port 15501. All SDKs are updated to use SynapRPC by default with HTTP as fallback.

SynapRPC advantages over RESP3: request multiplexing (no head-of-line blocking), binary
values natively, richer type system (u64, i64, f64, bool, bytes, map, array), pipeline
batching in a single frame.

Source: docs/analysis/synap-vs-redis/execution-plan.md Phase 2.1 (RESP2/RESP3) + Phase 2.4

## What Changes

### Server (synap-server)
- ADDED: `synap-server/src/protocol/resp3/` — RESP3 parser and writer; TCP listener on port 6379
- ADDED: `synap-server/src/protocol/synap_rpc/` — SynapRPC frame codec, request dispatcher; TCP listener on port 15501
- ADDED: `synap-server/src/protocol/mod.rs` — shared protocol infrastructure (command dispatch, auth)
- MODIFIED: `synap-server/src/main.rs` — spawn RESP3 and SynapRPC listeners alongside existing HTTP
- MODIFIED: `config.yml` + `ServerConfig` — add `resp3.enabled`, `resp3.port`, `synap_rpc.enabled`, `synap_rpc.port`

### SynapRPC Crate (new)
- ADDED: `synap-rpc/` workspace member — protocol definition crate shared by server and all Rust/SDK consumers
  - `synap-rpc/src/codec.rs` — frame encoder/decoder (4-byte length prefix + MessagePack body)
  - `synap-rpc/src/types.rs` — Request/Response types, SynapValue enum
  - `synap-rpc/src/client.rs` — async TCP client with connection pooling and multiplexing

### SDKs
- MODIFIED: `sdks/rust/` — replace HTTP transport with SynapRPC; HTTP retained as fallback via feature flag
- MODIFIED: `sdks/typescript/` — add TCP transport using Node.js `net.Socket`; auto-negotiate with server
- MODIFIED: `sdks/python/` — add TCP transport using `asyncio` streams; auto-negotiate
- MODIFIED: `sdks/csharp/` — add TCP transport using `System.Net.Sockets.TcpClient`
- MODIFIED: `sdks/php/` — add TCP transport using PHP `fsockopen`/`stream_socket_client`
- Each SDK: `SynapClient` gains `transport: "http" | "tcp" | "auto"` config (default `"auto"` = try TCP, fallback HTTP)

## Impact

- Affected specs: specs/protocol/spec.md
- Affected code: synap-server/src/protocol/ (new), synap-server/src/main.rs, config, all sdks/
- Breaking change: NO — HTTP API remains intact; TCP is additive; default SDK transport is auto-negotiated
- User benefit: GET/SET latency drops from ~150us to ~30-40us on localhost; Redis-compatible
  clients connect on port 6379 without changes; binary values work natively without base64
