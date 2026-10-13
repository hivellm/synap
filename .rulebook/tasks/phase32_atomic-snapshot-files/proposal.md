# Proposal: phase32_atomic-snapshot-files

Source: found during phase23 (`.rulebook/archive/2026-10-13-phase23_consistent-snapshots-and-expiry/design.md` §4, follow-ups recorded when phase31-33 were opened)
Target release: 1.4.0

## Why

- **Snapshots are written in place.** `SnapshotManager::write_image`
  (`crates/synap-server/src/persistence/snapshot.rs`) creates the final file
  `snapshot-v4-<unix seconds>.bin` and streams into it. A crash in the middle
  leaves a truncated file under the newest name. The name has one-second
  resolution, so a second snapshot in the same second truncates and rewrites
  the first one: a crash then leaves no complete copy of it.
- **A damaged newest snapshot wipes the data.** Recovery reads only the newest
  file. A truncated file fails with an I/O error and a checksum mismatch with
  `SnapshotCorrupted`; neither is in `PersistenceError::refuses_startup`, so
  `main.rs` logs "starting fresh" and serves EMPTY stores while the old
  snapshots and the WAL stay on disk. New writes are appended to the same WAL,
  the next snapshot records a cutoff past every old record, and retention then
  deletes the older valid snapshots: every write acknowledged before the crash
  is lost for good. No fallback to the previous snapshot exists.
- **The WAL can end up behind a snapshot.** The snapshot is fsynced, but the
  WAL records below its cutoff are only fsynced on the WAL's own schedule
  (`Periodic`: every second; `Never`: not at all). Power loss right after a
  snapshot leaves the snapshot holding effects whose records are gone from the
  WAL; after restart the WAL hands out offsets below the snapshot cutoff
  again, and the next recovery from that snapshot skips those new writes.
- **Retention is not ordered after durability.** Old snapshots are removed
  right after the new file is written, with no rename and no directory fsync,
  and `max_snapshots: 0` removes the snapshot just written.

## What Changes

- Snapshots are written to a temporary file in the snapshot directory,
  fsynced, renamed to their final name and the directory fsynced (Unix; on
  Windows the NTFS rename is atomic and journaled in order). A leftover
  temporary file from a crash is ignored by recovery and removed by the next
  snapshot.
- Snapshot file names carry a sequence number
  (`snapshot-v4-<20-digit sequence>.bin`), so names never collide and the load
  order does not depend on the clock. Files with the old names still load and
  count as older than any sequenced file.
- The WAL is fsynced up to the snapshot cutoff before the snapshot is
  published, whatever the fsync mode, and recovery makes the WAL continue at
  or above the cutoff of the snapshot it loaded.
- Recovery tries snapshots newest first. A damaged file (truncated, bad
  checksum, bad magic, undecodable) is skipped only when the WAL provably
  still holds every record after the older snapshot's cutoff; otherwise
  startup fails with an error that names the files and the reason, and that
  error refuses startup instead of starting empty. Lengths in a damaged file
  are checked against the file size before anything is allocated.
- Retention runs only after the new snapshot is durable, always keeps the
  snapshot just written, and a failed removal of an old file is a warning, not
  a failed snapshot. The server never truncates the WAL; this is documented.

## Impact

- Affected specs: persistence (snapshot write protocol, crash recovery, retention)
- Affected code: `crates/synap-server/src/persistence/` (snapshot, recovery,
  layer, types, wal_async, wal_frame), `crates/synap-server/src/main.rs`
- Breaking change: NO (old snapshot names and formats still load; 1.3.x could
  not read v4 snapshots already)
- User benefit: a crash at any byte of a snapshot write, or a damaged newest
  snapshot, never loses acknowledged writes; when it cannot be proven safe the
  server refuses to start instead of silently starting empty.
