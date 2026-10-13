# Snapshot format

Point-in-time snapshots are written by `SnapshotManager::write_image` (the
image is captured by `SnapshotImage::capture`) and read by `load_latest`
(`crates/synap-server/src/persistence/snapshot.rs`). The format is a single
streamed binary file ending in a CRC64 integrity digest.

## Layout (v5 — magic `SYNAP005`)

```
magic          : 8 bytes  ("SYNAP005"; "SYNAP004" = v4, "SYNAP003" = v3, "SYNAP002" = v2)
version        : u8       (5; must match the magic)
timestamp      : u64      (unix seconds)
wal_offset     : u64      (WAL cutoff: replay starts at this entry offset)
kv section     : count(u64) then [key_len(u32) key value_len(u32) value expires_at_ms(u64)]*
queue section  : count(u64) then [name_len(u32) name config_len(u32) bincode(QueueConfig)
                                  ready in_flight dead_letter]*                     ── v5
                 each message list: count(u64) then [len(u32) bincode(QueueMessageRecord)]*
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

A `QueueMessageRecord` holds every field of a queue message: `id`, `payload`,
`priority`, `retry_count`, `max_retries`, `created_at` (unix seconds) and
`headers`. `ready` keeps delivery order; `in_flight` lists the messages that
were delivered but not acknowledged, in the order they were delivered;
`dead_letter` is oldest first.

v4 differs only by its magic/version and its queue section, which held each
queue's ready messages only (`name msg_count [msg_len bincode(QueueMessage)]*`,
no configuration, no creation time). v3 also has no `expires_at_ms` field and
no HyperLogLog/bitmap sections; v2 also lacks the four collection sections.

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

v5 persists KV (with expiry), Queue (whole: configuration and every message
with its metadata), Stream, Hash, List, Set, Sorted-Set, HyperLogLog and
Bitmap. Recovery (`recovery.rs`) restores every datatype from the snapshot —
skipping keys whose deadline has passed and restoring the rest with their
absolute deadline — then replays the WAL from `wal_offset`, anchoring the
relative TTLs of pre-1.4.0 records at each entry's timestamp.

Queues are restored with `QueueManager::restore_queue`: every message keeps
its ID and metadata, dead letters stay dead-lettered, and in-flight messages
(their consumers did not survive the restart) become ready again, each ahead
of the waiting messages of the same or lower priority, in delivery order.
v4, v3 and v2 files load with their queues holding the ready messages (the
server's default queue configuration, creation time = load time), their keys
persistent where they had no expiry field, and empty sections for what they
lack.

Streams are additionally persisted by `StreamPersistence`; they are **not** written
to the KV WAL (audit M-014) — recovery never replayed those entries, so logging
them risked divergence. Their section is read when the file is written, outside
the barrier.

## Compatibility

1.4.0 reads v2, v3, v4 and v5 and writes v5. 1.3.x reads up to v3 and rejects
a v4 or v5 file, so a data directory that 1.4.0 has snapshotted cannot be
opened by 1.3.x.
