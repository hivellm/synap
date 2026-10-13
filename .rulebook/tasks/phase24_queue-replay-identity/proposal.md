# Proposal: phase24_queue-replay-identity

Source: docs/analysis/next-release-review-2026-10-12/ (finding D-05, priority P1)
Target release: 1.4.0

## Why

Recovery turns acknowledged queue messages into deliverable ones.
`apply_operation` (`persistence/apply.rs`) replays `QueuePublish` through the
ordinary `publish`, which builds a new `QueueMessage` with a freshly generated
ID (`synap-core/src/core/queue/manager.rs`, `queue/mod.rs`). The replayed
`QueueAck` carries the original ID, its error is ignored (`let _ =`), and
`ack` only removes from the pending set anyway. Snapshot restore
(`persistence/recovery.rs`) republishes messages the same way. After publish,
consume, ACK and a restart, the message is back in the ready state under a new
ID. Replicas use the same apply path, so master and replica IDs diverge and
ACKs replicated afterwards miss.

## What Changes

- Add a queue restore primitive that inserts a message with its original ID
  and metadata: priority, retry counters, delivery count and timestamps.
- WAL replay of `QueuePublish`, snapshot restore and replica apply use the
  restore primitive.
- Replayed ACK and NACK act by message ID whether the message is ready or
  pending. Only "not found" is tolerated, which is expected when the snapshot
  already excludes the message. Any other error fails recovery loudly.
- Master and replica keep identical message IDs.

## Impact

- Affected specs: queue (recovery)
- Affected code: `crates/synap-core/src/core/queue/`,
  `crates/synap-server/src/persistence/apply.rs`,
  `crates/synap-server/src/persistence/recovery.rs`
- Dependencies: phase20, phase23 (snapshot format v4 carries the message metadata)
- Breaking change: NO
- User benefit: an acknowledged message is never delivered again after a
  restart, and message IDs stay stable across restarts and replicas.
