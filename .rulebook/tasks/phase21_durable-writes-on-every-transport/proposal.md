# Proposal: phase21_durable-writes-on-every-transport

Source: docs/analysis/next-release-review-2026-10-12/ (finding D-08, priority P1, and finding N-01 added while turning the review into tasks, priority P1)
Target release: 1.4.0

## Why

- **N-01** (not in the original review; found while checking D-08): the RESP3
  and SynapRPC dispatchers apply writes to the in-memory stores and reply
  without calling the persistence layer. `cmd_set` in
  `crates/synap-server/src/protocol/resp3/command/kv.rs` and `SET` in
  `crates/synap-server/src/protocol/synap_rpc/dispatch/kv.rs` call
  `kv_store.set` and return. The only persistence call under
  `crates/synap-server/src/protocol/` is `log_transaction` in the EXEC
  handlers. Replication is fed by the persistence layer, so these writes never
  reach replicas either. With persistence enabled, every non-transactional
  write made over RESP3 or SynapRPC is lost on restart and missing on
  replicas.
- **D-08**: REST APPEND and SETRANGE (`server/handlers/kv.rs:400`, `:481`) and
  their envelope equivalents (`server/handlers/kv_cmd.rs:422`, `:494`) log
  `log_kv_set(key, vec![], None)`. Recovery and replicas replace the value with
  empty bytes and drop its expiry.

Both come from each transport deciding on its own what to log. Transactions
already show the fix: record each command's resulting effect once, through
one path.

## What Changes

- One mutation path, shared by every transport, records a command's effect
  through the persistence layer (WAL and replication) before the command is
  acknowledged. The same command produces the same `Operation` whatever
  transport carried it. Derived results (INCR, APPEND, SETRANGE, ...) are
  recorded as their resulting effect, as transactions already do.
- APPEND and SETRANGE return the resulting value and expiry from the store
  under the key's shard lock, and that value is what gets logged.
- RESP3, SynapRPC, REST, the envelope and MCP all write through this path.
- A parity test runs every mutating command on every transport, restarts from
  the WAL, and compares the recovered state and a replica's state with the
  original.
- Logging failures fail the command, consistent with phase20.

Same-key ordering between concurrent writers is handled in phase23, which
covers every write path at once.

## Impact

- Affected specs: persistence (write durability)
- Affected code: `crates/synap-server/src/protocol/resp3/command/`,
  `crates/synap-server/src/protocol/synap_rpc/dispatch/`,
  `crates/synap-server/src/server/handlers/`,
  `crates/synap-server/src/server/mcp_handlers.rs`,
  `crates/synap-server/src/persistence/layer.rs`,
  `crates/synap-core/src/core/kv_store/`
- Dependencies: phase20 (writer errors and flush barrier)
- Breaking change: NO on the wire. Native writes gain WAL and replication
  cost, which phase29 measures.
- User benefit: a write acknowledged on any transport survives restart and
  reaches replicas, with its exact bytes.
