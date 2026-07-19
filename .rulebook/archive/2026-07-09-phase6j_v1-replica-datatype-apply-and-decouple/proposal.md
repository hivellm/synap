# Proposal: phase6j_v1-replica-datatype-apply-and-decouple

Source: docs/analysis/synap-audit/ (M-005 completion); follow-up of phase6e

## Why
phase6e closed the critical M-005 hole: replication is now instantiated by the
running server and the master propagates every logged operation to replicas.
Three gaps remain: (1) the replica's `apply_operation` only handles KV and stream
operations, so hash/list/set/sorted-set/queue writes reach the replica but are
not applied — the replica diverges for those datatypes; (2) propagation
piggybacks on the persistence log, so replication requires persistence enabled;
(3) replication status (role, connected replicas, lag) is not surfaced in INFO.

## What Changes
1. Extend `ReplicaNode` to hold the hash/list/set/sorted-set/queue stores and
   apply every `Operation` variant (mirroring `recovery.rs` replay), so a replica
   converges to the master for all datatypes.
2. Decouple replication propagation from the persistence log so replication works
   with persistence disabled (a shared propagate hook used by both WAL and
   replication).
3. Surface replication status in INFO/metrics (role, connected replicas, lag).
4. End-to-end tests: a master–replica pair converges for every datatype, and a
   reconnect triggers the correct partial/full resync.

## Impact
- Affected specs: full-datatype replication + status (ADDED/MODIFIED)
- Affected code: `crates/synap-server/src/replication/replica.rs` (+ its
  constructor and main.rs wiring), the write/propagate path, `server/handlers`
  INFO, `monitoring`
- Breaking change: NO (extends existing behaviour)
- User benefit: a replica that faithfully mirrors the master for every datatype,
  usable for read-scaling and failover
