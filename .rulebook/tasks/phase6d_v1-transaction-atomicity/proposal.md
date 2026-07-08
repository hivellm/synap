# Proposal: phase6d_v1-transaction-atomicity

Source: docs/analysis/synap-audit/ (M-008, M-010)

## Why
MULTI/EXEC advertises atomicity it does not deliver. (M-008) The module doc claims "Atomic
execution with sorted multi-key locking" (`core/transaction.rs:3-9`), but `execute_transaction`
computes `keys_to_lock` and then calls `execute_commands` without holding any lock — the
honest inline comment says "For simplicity, we'll use a single lock on all keys / In production,
you'd use sorted locks per key" (`transaction.rs:350-351`). Each command inside the block takes
and releases its own per-shard lock (`transaction.rs:403+`), so concurrent EXECs interleave and
the WATCH version bump happens only after execution (`transaction.rs:360-368`) — the check-and-set
guarantee Redis users depend on is violated. (M-010) Transaction commands call the core stores
directly (`transaction.rs:411-449`), bypassing the WAL/persistence layer (invoked only in the
HTTP handlers, `persistence/layer.rs:46-59`) and replication — a committed EXEC can be lost on
crash and is never replicated.

## What Changes
1. Hold the sorted per-key locks (or a single serialized transaction executor) for the entire
   duration of `execute_commands`, so no other writer interleaves within an EXEC and the WATCH
   version check + apply are one critical section (closing the TOCTOU window).
2. Route every transaction command through the WAL/persistence path (ideally as one atomic WAL
   batch per EXEC) and through replication, so committed transactions are durable and replicated
   exactly like single writes.
3. Update the module documentation to match the real (now correct) guarantees.

## Impact
- Affected specs: transaction atomicity + durability (MODIFIED)
- Affected code: `crates/synap-core/src/transaction.rs` (post-restructure),
  the WAL batch API in `persistence/`, and the write path shared with handlers
- Breaking change: NO in API; behavior becomes correctly atomic and durable
- User benefit: MULTI/EXEC/WATCH behaves like Redis — isolated, all-or-nothing, durable,
  and replicated
