# Proposal: phase22_kv-watch-server-endpoints

Source: docs/analysis/kv-watch-observable/ (F-001, F-003, F-011)

## Why

Phase 21 gives the server a value-carrying watch publish path, but clients have no ergonomic
surface to consume it. The SynapRPC push bridge (`synap_rpc/server.rs:122-199`) already
delivers pub/sub messages as push frames, and the `/kv/ws` WATCH WebSocket endpoint is
routed but returns 501 NOT_IMPLEMENTED — its stated blocker ("KVStore change notifications")
is solved by phase 21. This phase exposes watch over both protocols.

## What Changes

- SynapRPC: `WATCH <pattern> [mode]` / `UNWATCH <pattern>` commands in the dispatch layer,
  resolving to SUBSCRIBE/UNSUBSCRIBE on `__watch@0__:<pattern>` and riding the existing push
  bridge. `mode` is `value` (default) or `notify` (envelope without inline value). Wildcard
  patterns (`user:*`) are supported via the router's existing wildcard matching.
- WebSocket: complete `kv_websocket` (`server/handlers/websocket.rs:7-37`) mirroring
  `handle_pubsub_socket` (431-561): honor `?keys=` (comma-separated, wildcards allowed),
  stream watch envelopes as JSON frames, existing slow-consumer semantics apply.
- Config: `watch.max_inline_value_bytes` in config.yml (default 65536) with
  `SYNAP_WATCH_MAX_INLINE_VALUE_BYTES` env override, threaded into the phase-21 notifier.

## Impact

- Affected specs: specs/kv-watch/spec.md (ADDED endpoint requirements)
- Affected code: crates/synap-server/src/protocol/synap_rpc/dispatch/,
  crates/synap-server/src/server/handlers/websocket.rs, crates/synap-server/src/server/router.rs,
  crates/synap-server/src/config.rs, crates/synap-server/src/main.rs
- Breaking change: NO (the 501 stub becomes functional)
- User benefit: any RPC or WebSocket client can watch keys and receive value broadcasts,
  with backpressure and wildcard support inherited from pub/sub.
