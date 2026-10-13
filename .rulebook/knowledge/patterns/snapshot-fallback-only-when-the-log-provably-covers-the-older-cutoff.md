# Snapshot fallback only when the log provably covers the older cutoff; publish via temp + fsync + rename

**Category**: persistence
**Tags**: persistence, snapshot, wal, fsync, crash-safety, phase32

## Description

A snapshot that pairs with a write-ahead log needs three rules to survive a
crash at any byte:

1. **Publish atomically.** Write `<final>.tmp` in the same directory, fsync it,
   rename to the final name, fsync the directory (Unix; NTFS renames are atomic
   and journaled in order). Recovery never lists temp files; the next write
   deletes leftovers. Retention runs only after the rename is durable and never
   deletes the file just written (treat `max = 0` as 1).
2. **Name by sequence, not by clock.** One past the highest sequence in the
   directory, under a write lock, with snapshots serialized so a later name
   always has a later cutoff. Second-resolution timestamps collide and NTP steps
   reorder them.
3. **Fall back only on proof.** Try files newest first; a damaged file
   (truncated, bad checksum/magic, undecodable) may be skipped only if the log
   still holds every record from the older snapshot's cutoff: log enabled,
   first record at or before the cutoff (offsets start at 1), no offset break
   after the cutoff, and records present if that snapshot came after some.
   Otherwise fail with an error that refuses startup and names the files. A
   non-damage I/O error (permissions) also refuses: the file may be intact.

Also make the log durable up to the cutoff before publishing the snapshot
(force an fsync even in "never fsync" mode), and after recovery raise the
log's next offset to at least the loaded cutoff: otherwise a power loss right
after a snapshot leaves the log behind it, and new records reuse offsets the
next recovery skips.

When reading a damaged file, bound every length by the bytes left in the file
before allocating, so damage surfaces as "corrupted" instead of an
out-of-memory abort, and map EOF/InvalidData to the corruption error so the
caller can classify it.

## Example

```rust
for path in files.into_iter().rev() {
    match mgr.load(&path).await {
        Ok(snapshot) => return Ok(Choice { loaded: Some((snapshot, path)), damaged }),
        Err(PersistenceError::SnapshotCorrupted(_)) => damaged.push(path),
        Err(e) => return Err(SnapshotUnrecoverable(format!("cannot read {path:?}: {e}"))),
    }
}
// then: if !damaged.is_empty() { check WAL coverage of the chosen cutoff or refuse }
```
