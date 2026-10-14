# Replay settles by ID in any state when some transitions are not logged

**Category**: persistence
**Tags**: persistence, wal, replication, queue, identity, phase24

## Description

Queue deliveries (consume) are not logged, so during WAL replay and on a
replica a delivered message is still "ready". Two things then break unless the
replay is designed for it: (1) a replayed publish that mints a new ID can never
be matched by the ACK that names the original ID, and (2) a settle that only
looks in the "delivered" set finds nothing. The fix has three parts: restore
the published message exactly (original ID and every field; keep a persisted
record type when the API serde form drops a field such as `created_at`), make
replayed ACK/NACK act by ID on ready OR pending messages, and record bulk
removals by the IDs they removed (QPURGE logs the purged IDs) because "clear
the ready list" replays to a different result when the replayed ready list
also holds the undelivered-in-the-log messages. Tolerate only not-found on
replay (a snapshot taken after the settle no longer holds the message) and
fail on anything else. Unlogged background transitions that change metadata
(ACK-deadline requeues raising retry counters) must be recorded too, as the
operation they are equivalent to (a NACK), under the same ordering stripe.

Test it end to end: publish, consume, ACK, restart (WAL-only and
snapshot+WAL), then drain the queue as a consumer would; and compare master
and replica queue state field by field (IDs are UUIDs, so any re-minting shows).

## Example

// replay
Operation::QueueAck { queue, message_id } =>
    tolerate_not_found(qm.replay_ack(&queue, &message_id).await)?,
Operation::QueuePurge { queue, message_ids } =>
    tolerate_not_found(qm.remove_messages(&queue, &message_ids).await.map(|_| ()))?,

## When to Use

Any log/replication stream where some state transitions are deliberately not
recorded (deliveries, reads that move state) but later records refer to
entities by identity.

## When NOT to Use

Logs that record every transition, or entities that are addressed only by key
(re-applying the value is enough).
