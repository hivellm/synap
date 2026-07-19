# Proposal: phase3_full-rpc-resp3-parity-and-url-schemes

## Why

Today SynapRPC and RESP3 only expose the KV-family commands (KV, Hash,
List, Set, SortedSet, Bitmap, HyperLogLog partial, Bitmap/Geospatial
partial). Every other subsystem — queues, streams, pub/sub,
transactions, Lua scripting, auth/admin, kv stats — still forces the
SDK to fall back to HTTP REST. Three structural problems follow:

1. Users who pick a binary transport for latency silently pay HTTP
   framing cost for half their calls, with no way to audit which calls
   fell back.
2. The SDK constructor always demands an HTTP `base_url` even when the
   application never intends to speak HTTP — making "use SynapRPC" feel
   like an HTTP extension instead of a first-class transport.
3. Connection config is a multi-field builder (`transport`, `rpcHost`,
   `rpcPort`, `resp3Host`, `resp3Port`, `base_url`) in every language,
   doubling the API surface per SDK.

This task closes the gap: every command available over HTTP becomes
available natively over SynapRPC **and** RESP3, and clients are
configured with a single connection URL using one of three schemes:

- `http://host:port` / `https://host:port` — REST transport (today's behaviour)
- `synap://host:port` — SynapRPC binary transport (MessagePack over TCP)
- `resp3://host:port` — RESP3 Redis-compatible transport

After this task the silent HTTP fallback is removed from binary
transports: unknown commands raise a typed error instead of quietly
switching protocols.

## What Changes

### Server (synap-server)

Extend the SynapRPC dispatcher (`protocol/synap_rpc/server.rs` + its
command handler) and the RESP3 command router to recognise and execute
the ~40 commands that today only exist on the HTTP router:

- Queue: create, delete, list, publish, consume, ack, nack, stats
- Stream: create_room, publish, read, replay, delete_room, list_rooms,
  stats, consumer-group ops
- Pub/Sub: publish, subscribe, unsubscribe, topics
- Transaction: multi, exec, discard, watch, unwatch, scoped ops
- Script: eval, evalsha, load, exists, flush, kill
- HyperLogLog: pfadd, pfcount, pfmerge, stats
- Geospatial: geoadd, geopos, geodist, geohash, georadius,
  georadiusbymember, geosearch, stats
- KV: kv.stats (the remaining HTTP-only KV call)

Streaming commands use server-push frames on SynapRPC and standard
`SUBSCRIBE` / `XREAD BLOCK` semantics on RESP3. MCP and UMICP remain
HTTP-only and are out of scope.

### SDKs (Rust, TypeScript, Python, PHP, C#)

1. Accept a single connection URL using the three schemes above; the
   scheme drives the transport. No separate `rpcHost`, `resp3Port`,
   `base_url` fields are required. Legacy builder methods remain as
   deprecated aliases for one release.
2. Add native mapper entries for every new server command, mirrored
   across all five languages.
3. Remove the silent HTTP fallback from binary-transport code paths.
   Unknown commands raise `UnsupportedCommandError` (or language
   equivalent) with the command name and the active transport mode.
4. Reactive/streaming: `pubsub.subscribe`, `stream.observe_events`, and
   the reactive queue consumer work over SynapRPC and RESP3 without
   touching HTTP.

### Tests

- Expand the Rust E2E suite so every transport exercises queues,
  streams, pub/sub, transactions and scripts alongside KV/Hash/List.
- Mirror the same coverage in the TypeScript E2E suite.
- Add comparable S2S/E2E coverage in Python, PHP and C# SDKs.
- Server-side integration tests asserting RPC and RESP3 dispatchers
  produce the same result as the HTTP route for each command.

### Documentation

- Root `README.md` and `CHANGELOG.md`.
- Every SDK `README.md` and `CHANGELOG.md` — rewrite the Transports
  section to show URL schemes and queue/stream/pub-sub examples running
  over `synap://`.

## Impact

- **Affected specs**: `protocol/synap_rpc`, `protocol/resp3`,
  new `specs/transports/spec.md` capturing the URL-scheme contract
  and the "no silent fallback" rule.
- **Affected code**: `synap-server/src/protocol/synap_rpc/**`,
  `synap-server/src/protocol/resp3/**`, `sdks/rust/src/{client,transport}.rs`,
  `sdks/typescript/src/{client,transport,types}.ts`,
  `sdks/python/synap_sdk/{client,config,transport}.py`,
  `sdks/php/src/{SynapClient,SynapConfig,Transport}.php`,
  `sdks/csharp/src/Synap.SDK/{SynapClient,SynapConfig,Transport}.cs`.
- **Breaking change**: YES — bump to `0.11.0`. Callers using the
  multi-field builder still work (deprecated). Callers that relied on
  silent HTTP fallback for unsupported commands now see a typed error;
  every subsystem above is implemented in the same release so the
  failure mode is limited to genuinely HTTP-only endpoints (MCP/UMICP).
- **User benefit**: one obvious way to pick a transport; pure-binary
  clients can drop the HTTP client dependency; no silent fallback means
  latency is honest; benchmarks become simpler because all paths run on
  the same transport.

## Source

Direct follow-up to the 0.10.0 transport work — commits `57353f9`
(multi-transport SDKs) and `bc85bfd` (v0.10.0 release).
