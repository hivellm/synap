# Snapshot format

Point-in-time snapshots are written by `SnapshotManager::write_image` (the
image is captured by `SnapshotImage::capture`) and read by `load_latest`
(`crates/synap-server/src/persistence/snapshot.rs`). The format is a single
streamed binary file ending in a CRC64 integrity digest.

## Layout (v4 — magic `SYNAP004`)

```
magic          : 8 bytes  ("SYNAP004"; "SYNAP003" = v3, "SYNAP002" = v2)
version        : u8       (4; must match the magic)
timestamp      : u64      (unix seconds)
wal_offset     : u64      (WAL cutoff: replay starts at this entry offset)
kv section     : count(u64) then [key_len(u32) key value_len(u32) value expires_at_ms(u64)]*
queue section  : count(u64) then [name_len(u32) name msg_count(u64) [msg_len(u32) bincode(msg)]*]*
stream section : count(u64) then [name_len(u32) name event_count(u64) [ev_len(u32) bincode(event)]*]*
hash section   : map section (value = bincode(HashMap<field,value>))   ── v3+
list section   : map section (value = bincode(ListValue))              ── v3+
set section    : map section (value = bincode(SetValue))               ── v3+
sortedset sect : map section (value = bincode(Vec<(member,score)>))    ── v3+
hll section    : map section (value = bincode(HyperLogLogValue))       ── v4+
bitmap section : map section (value = bincode(BitmapValue))            ── v4+
checksum       : u64      (CRC64 over every preceding byte)
```

A *map section* is `count(u64)` then, per entry, `key_len(u32) key data_len(u32) bincode(value)`.
Numbers are written big-endian.

`expires_at_ms` is the key's absolute expiry in Unix milliseconds, or `0` for
a key without expiry (0 is never a future deadline). `HyperLogLogValue` and
`BitmapValue` carry their own `created_at` (unix seconds) and `ttl_secs`, so
their expiry `created_at + ttl_secs` is absolute too.

v3 differs only by its magic/version and by having no `expires_at_ms` field
and no HyperLogLog/bitmap sections; v2 also lacks the four collection
sections.

## Integrity

The writer feeds a running CRC64 the same byte sequence it writes, except that
numeric length/count fields are digested little-endian (raw bytes for
keys/values). On load the digest is recomputed identically and compared to
the trailing checksum; a mismatch returns
`PersistenceError::SnapshotCorrupted` instead of loading corrupt data (audit
M-002).

## Pairing with the WAL

`wal_offset` is exact. `PersistenceLayer::snapshot` holds the write barrier
(every ordering stripe, see `persistence/order.rs`) while it places a mark in
the WAL writer queue and captures the image; every write applied before the
barrier has its record ahead of the mark (offset below `wal_offset`) and its
effect in the image, every write after it is outside the image with an offset
at or above `wal_offset`. Recovery replays from `wal_offset`, so each write is
applied exactly once. Serialization happens after the barrier is released.

## Datatype coverage and recovery

v4 persists KV (with expiry), Queue, Stream, Hash, List, Set, Sorted-Set,
HyperLogLog and Bitmap. Recovery (`recovery.rs`) restores every datatype from
the snapshot — skipping keys whose deadline has passed and restoring the rest
with their absolute deadline — then replays the WAL from `wal_offset`, anchoring
the relative TTLs of pre-1.4.0 records at each entry's timestamp. v3 and v2
files load with their keys persistent and empty sections for what they lack.

Streams are additionally persisted by `StreamPersistence`; they are **not** written
to the KV WAL (audit M-014) — recovery never replayed those entries, so logging
them risked divergence. Their section is read when the file is written, outside
the barrier.

## Compatibility

1.4.0 reads v2, v3 and v4. 1.3.x reads up to v3 and rejects a v4 file, so a
data directory that 1.4.0 has snapshotted cannot be opened by 1.3.x.
