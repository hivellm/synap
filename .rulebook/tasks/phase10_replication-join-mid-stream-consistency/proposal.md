# Proposal: phase10_replication-join-mid-stream-consistency

Source: GitHub issue #234 (deferred out of phase6 v1.0 hardening)

## Why
A replica that connects while the master is actively writing loses the writes
that land between the snapshot point and the moment the replica starts receiving
the live operation stream. `test_concurrent_writes_during_sync`
(crates/synap-server/tests/replication_integration.rs) starts a replica mid-write
of 500 keys and never reaches 500 on the replica — it fails deterministically and
was mislabeled a flaky-timing test. The master takes the snapshot at offset X and
only registers the replica in the broadcast set afterwards; any operation
replicated in that window is neither in the snapshot nor delivered to the replica.

## What Changes
1. Close the join race in `MasterNode::handle_replica_connection`: register the
   replica in the broadcast set (so it starts capturing live ops) BEFORE taking
   the snapshot, then send the full sync, then flush any operations from the
   replication log with offset > snapshot offset before entering the live stream
   loop — so no operation in `(snapshot_offset, current]` is dropped.
2. The replica applies operations idempotently (SET/DEL replay is safe) so a
   small overlap between the log flush and the live stream is harmless.
3. Keep `test_concurrent_writes_during_sync` un-ignored and assert the replica
   reaches all 500 keys.

## Impact
- Affected specs: replication consistency on replica join (MODIFIED)
- Affected code: crates/synap-server/src/replication/master.rs (connection
  handler), replica.rs (offset handling if needed)
- Breaking change: NO
- User benefit: a replica joining a live master converges to the full dataset
  with no lost writes
