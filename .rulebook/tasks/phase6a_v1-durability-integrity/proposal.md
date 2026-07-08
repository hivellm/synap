# Proposal: phase6a_v1-durability-integrity

Source: docs/analysis/synap-audit/ (M-001, M-002, M-014)

## Why
The persistence layer has data-integrity gaps that make a 1.0 durability promise false.
(M-001) The streaming snapshot writer serializes only KV, Queue and Stream data
(`persistence/snapshot.rs:26-203`); the loader hardcodes `list_data/set_data/sorted_set_data`
to empty (`snapshot.rs:338-340`) and recovery seeds an empty `HashStore` (`recovery.rs:107`).
Because a snapshot advances the WAL replay baseline (`wal_offset`), any Hash/List/Set/SortedSet
write made before the snapshot is permanently lost on restart. (M-002) The snapshot computes
and writes a CRC64 but the loader discards it (`snapshot.rs:320` `let _checksum = ... unwrap_or(0)`)
— corrupt snapshots load silently. (M-014) `StreamPublish` is written to the WAL
(`persistence/layer.rs:408-431`) but its replay is a no-op (`recovery.rs:193-202`), so WAL space
is wasted and the two stream-persistence paths can diverge.

## What Changes
1. Extend the streaming snapshot format to include Hash, List, Set and SortedSet sections,
   symmetrical writer (`snapshot.rs create_snapshot`) and loader (`snapshot.rs load_latest`),
   bumping `SNAPSHOT_VERSION` and keeping backward-compatible reads of the old format.
2. Populate `HashStore`/`ListStore`/`SetStore`/`SortedSetStore` from the snapshot in
   `recovery.rs` instead of seeding empty stores.
3. Verify the CRC64 on load: recompute while streaming and return `SnapshotCorrupted` on
   mismatch instead of discarding `_checksum`.
4. Resolve the stream/WAL duplication (M-014): either replay `StreamPublish` in `recovery.rs`
   against the `StreamManager`, or stop logging streams to the WAL and rely solely on
   `StreamPersistence` — pick one path and document it.

## Impact
- Affected specs: snapshot format (MODIFIED — version bump)
- Affected code: `crates/synap-server/src/persistence/snapshot.rs`, `persistence/recovery.rs`,
  `persistence/layer.rs`, `persistence/types.rs` (Snapshot struct)
- Breaking change: YES (snapshot format version bump; old snapshots read via compat path)
- User benefit: restart no longer loses hash/list/set/sorted-set data; corrupt snapshots are
  detected instead of loaded silently
