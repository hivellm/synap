# Replica join-mid-stream: register-before-snapshot + offset dedup, and a partial-sync framing bug (#234)
**Source**: manual
**Date**: 2026-07-10
**Related Task**: phase10_replication-join-mid-stream-consistency
**Tags**: replication, phase10, issue-234, framing, master-replica, rust
Issue #234: a replica joining while the master writes converged to 0 keys (test_concurrent_writes_during_sync). Two bugs:

1. Framing bug (primary): MasterNode::send_partial_sync wrote the PartialSync command with a raw `stream.write_all(&bincode)` — NO length prefix — while send_full_sync used send_command (4-byte BE length prefix) and the replica's read_command reads a length-prefixed frame. So a partial sync (taken whenever needs_full_sync is false, e.g. replica requests offset 0 but master current_offset>0) was misframed and the replica applied nothing. Fix: send_partial_sync calls Self::send_command(stream, &cmd). Passing full-sync tests hid it because they took the full-sync path.

2. Join race (the documented gap): the master registered the replica in the fan-out `replicas` map AFTER taking the snapshot, so writes between snapshot and registration were neither in the snapshot nor streamed. Fix: register the replica (insert its mpsc sender) BEFORE send_full_sync/send_partial_sync. Once registered, replication_task fans every live op into the replica's unbounded channel, which buffers during snapshot transfer; the stream loop drains it after the sync frame, so wire order stays snapshot-then-ops.

3. Dedup for the overlap: register-before-snapshot means the replica may receive ops the snapshot already covers. In ReplicaNode::apply_operation, if op.offset < current_offset (snapshot offset), return Ok(()) without applying — so non-idempotent ops (list push, queue publish) aren't doubled. Offsets: ReplicationLog.append returns fetch_add(1) (0-based op offset); current_offset() = count = next offset; snapshot at offset X covers ops < X.

Residual caveat: kv_store mutation and log.append aren't atomic (handler mutates store then calls replicate/append), so the snapshot boundary is fuzzy by a few ops — safe for idempotent KV SET/DEL (the test), minor duplication risk for non-idempotent ops exactly at the boundary; documented in replication.md.

Test: test_concurrent_writes_during_sync un-ignored, now asserts the replica converges to all 500 keys. Diagnose replication issues by printing replica.stats() (connected/replica_offset/total_replicated) — total_replicated=0 means apply_operation never ran (framing/delivery issue), not a logic bug.