# Proposal: phase31_replica-full-sync-all-datatypes

Source: found during phase23 and recorded as out of scope there
(`.rulebook/archive/2026-10-13-phase23_consistent-snapshots-and-expiry/design.md` §4).

## Why

A replica's full sync (`replication/master.rs::send_full_sync` +
`replication/sync.rs`) ships only KV keys and stream events, so a new or
resyncing replica never receives the hashes, lists, sets, sorted sets (and
geo), HyperLogLogs, bitmaps or queues the master held before it connected.
The image is also not taken under the phase23 write barrier: the master reads
the replication offset and then dumps the stores, so a write that lands in
between is either in the image and streamed again (applied twice: LPUSH,
HINCRBY, INCR...) or in neither. A fresh replica that connects after the
master's first write since boot gets a partial sync from offset 0 instead of
a full sync, which misses every key the master recovered from its WAL or
snapshot. Finally, `MasterNode::replicate` assigns the replication offset and
queues the record for fan-out in two unsynchronized steps, so two writers to
different keys can reach a replica out of offset order, and the replica's
offset deduplication then drops the lower one.

## What Changes

- The full-sync image carries every replicated datatype: KV (with absolute
  expiry), hashes, lists, sets, sorted sets (geo included), HyperLogLogs,
  bitmaps and queues, plus stream events as before.
- The master captures the image under the phase23 barrier: it locks every
  `WriteOrder` stripe, reads the replication offset, copies the stores with
  the snapshot's `SnapshotImage::capture`, releases the stripes and serializes
  outside them. The `WriteOrder` is shared between the persistence layer and
  the master node, so the barrier covers every write the master replicates.
- The image travels in a new, versioned `ReplicationCommand::FullSyncImage`
  (version 2, CRC32 checked). A replica refuses an image version it does not
  know with an error naming both versions; it still accepts the 1.3.x
  `FullSync` (KV + streams) so replicas can be upgraded before the master.
- The replica validates and decodes the whole image before touching its
  stores, then clears every store the image covers and loads it on its
  replication task, before any streamed command is applied (clear then load).
- The master always full-syncs a replica that asks for offset 0 or for an
  offset beyond its log; `replicate` assigns the offset and queues the record
  for fan-out in one critical section, so replicas receive records in offset
  order.
- Docs: `docs/features/REPLICATION.md`, `docs/users/configuration/REPLICATION.md`.

## Impact

- Affected specs: `specs/replication/spec.md`
- Affected code: `crates/synap-server/src/replication/{master.rs,replica.rs,sync.rs,types.rs,replication_log.rs}`,
  `crates/synap-server/src/persistence/{layer.rs,snapshot.rs}`,
  `crates/synap-server/src/main.rs`, `crates/synap-core/src/core/{hash.rs,set.rs,hyperloglog.rs,list/store.rs,sorted_set/store.rs,bitmap/mod.rs}`
- Breaking change: YES for mixed versions only — a pre-1.4.0 replica cannot
  decode the new full-sync command; upgrade replicas before the master (same
  order phase22 already requires).
- User benefit: a replica that connects or resyncs holds exactly the master's
  data for every datatype, with no write applied twice or lost.
- Target release: 1.4.0
