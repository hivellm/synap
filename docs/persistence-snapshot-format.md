# Snapshot format

Point-in-time snapshots are written by `SnapshotManager::create_snapshot` and read
by `load_latest` (`crates/synap-server/src/persistence/snapshot.rs`). The format is
a single streamed binary file ending in a CRC64 integrity digest.

## Layout (v3 — magic `SYNAP003`)

```
magic         : 8 bytes  ("SYNAP003"; "SYNAP002" = v2, no collection sections)
version        : u8       (3)
timestamp      : u64      (unix seconds)
wal_offset     : u64      (WAL replay baseline)
kv section     : count(u64) then [key_len(u32) key value_len(u32) value]*
queue section  : count(u64) then [name_len(u32) name msg_count(u64) [msg_len(u32) bincode(msg)]*]*
stream section : count(u64) then [name_len(u32) name event_count(u64) [ev_len(u32) bincode(event)]*]*
hash section   : map section (value = bincode(HashMap<field,value>))   ── v3+
list section   : map section (value = bincode(ListValue))              ── v3+
set section    : map section (value = bincode(SetValue))               ── v3+
sortedset sect : map section (value = bincode(Vec<(member,score)>))    ── v3+
checksum       : u64      (CRC64 over every preceding byte)
```

A *map section* is `count(u64)` then, per entry, `key_len(u32) key data_len(u32) bincode(value)`.

## Integrity

The writer feeds a running CRC64 the same byte sequence it writes (LE for the
numeric length/count fields, raw bytes for keys/values). On load the digest is
recomputed identically and compared to the trailing checksum; a mismatch returns
`PersistenceError::SnapshotCorrupted` instead of loading corrupt data (audit M-002).

## Datatype coverage

v3 persists KV, Queue, Stream, Hash, List, Set and Sorted-Set. Earlier v2 files
load with empty collection maps for backward compatibility. Recovery
(`recovery.rs`) restores every datatype from the snapshot, then replays the WAL
from `wal_offset`.

Streams are additionally persisted by `StreamPersistence`; they are **not** written
to the KV WAL (audit M-014) — recovery never replayed those entries, so logging
them risked divergence.
