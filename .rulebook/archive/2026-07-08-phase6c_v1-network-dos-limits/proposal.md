# Proposal: phase6c_v1-network-dos-limits

Source: docs/analysis/synap-audit/ (M-006, M-007, M-011, M-015)

## Why
The network layer allocates attacker-controlled amounts of memory with no ceiling, so a single
connection can OOM the process. (M-006) The RESP3 parser does `vec![0u8; len]` with a
client-supplied bulk length and `Vec::with_capacity(count)` with a client-supplied array count,
both uncapped (`protocol/resp3/parser.rs:104,150`; `read_line` is unbounded too). (M-007) The
SynapRPC codec reads a 4-byte length prefix then `vec![0u8; len]` with no ceiling — up to ~4 GB
per frame (`protocol/synap_rpc/codec.rs:76-80`). (M-011) Pub/Sub delivers over an
`mpsc::UnboundedSender` (`core/pubsub.rs:21`), so one slow/stuck subscriber grows its channel
without bound until the server OOMs. (M-015) The accept loops spawn a task per connection with
no maximum-connections limit. Redis caps all of these (`proto-max-bulk-len`,
`client-output-buffer-limit`, `maxclients`).

## What Changes
1. Enforce a configurable max bulk length and max array/element count in the RESP3 parser;
   reject oversized frames before allocating; bound `read_line` length.
2. Enforce a configurable max frame size in the SynapRPC codec `read_frame`; drop the
   connection when exceeded.
3. Replace the pub/sub unbounded channel with a bounded channel plus a per-subscriber
   output-buffer limit and a drop-or-disconnect policy; expose a slow-consumer metric.
4. Add a configurable max-connections limit (semaphore-bounded accept) and an idle-connection
   timeout to both binary listeners.
5. Surface all limits in config with safe defaults (documented in config.example.yml).

## Impact
- Affected specs: protocol resource limits (ADDED)
- Affected code: `crates/synap-server/src/protocol/resp3/parser.rs`,
  `protocol/synap_rpc/codec.rs`, `protocol/synap_rpc/server.rs`, `protocol/resp3/server.rs`,
  `core/pubsub.rs`, `config.rs`, `config.example.yml`
- Breaking change: NO for well-behaved clients; oversized frames/floods are now rejected
- User benefit: a single malicious or buggy client can no longer exhaust server memory or FDs
