# Proposal: phase34_wal-unreadable-tail-and-stream-retention-zero

Source: found by the phase25 agent while fixing review findings
(phase25_migrator-server-format-parity)
Target release: 1.4.0

## Why

Two paths silently destroy data the server acknowledged.

1. **WAL tail that cannot be decoded.** Opening the WAL at startup
   (`persistence/wal_frame.rs`: `prepare` calls `repair`) truncates every
   invalid tail the scan reports, whatever `TailKind` it is. phase25 typed the
   tail (`Incomplete`, `ChecksumMismatch`, `Blank`, `Unreadable`) but only made
   `synap-migrate` refuse `Unreadable`. An `Unreadable` tail is a complete frame
   whose checksum matches but whose payload does not decode: a crash cannot
   leave one. It is a record this build cannot read, written by a newer
   release or exposed by a decoding bug, and truncating it deletes an
   acknowledged write without a trace beyond a warning.
2. **Stream retention of 0.** `StreamConfig::retention_secs` documents `0` as
   infinite retention, but `Room::compact` (`synap-core` `core/stream.rs`)
   computes its cutoff as `now - 0` and drops every event. Since phase25,
   startup restores stream rooms from the newest snapshot, so a room configured
   to keep events forever is emptied by the first compaction after a restart
   (and by every compaction before it).

## What Changes

- Opening the WAL refuses to start, without modifying the file, when the scan
  ends at an `Unreadable` record, as it already refuses interior corruption
  (phase22 policy). The error names the WAL path, the byte offset of the record
  and that a newer Synap release may have written it. The tails a crash leaves
  (`Incomplete`, `ChecksumMismatch`, `Blank`) are still truncated and startup
  continues.
- A new `PersistenceError::WALUnreadableRecord` carries that error and is one
  of the errors `refuses_startup` stops the server on.
- `Room::compact` skips time-based retention when `retention_secs` is `0`. The
  buffer caps (`max_buffer_size`, `max_unread_buffer_size`) are enforced on
  publish and restore, not by compaction, so they apply unchanged.
- When compaction removes events, the room's earliest offset and statistics
  follow, also when it removes every event, so the next publish is readable at
  its offset and a read below the earliest retained offset is out of range.
- Documentation of WAL startup checks and of stream retention matches the code.

## Impact

- Affected specs: persistence, streams
- Affected code: `crates/synap-server/src/persistence/wal_frame.rs`,
  `crates/synap-server/src/persistence/types.rs`,
  `crates/synap-core/src/core/stream.rs`, their tests, and the persistence and
  stream documentation
- Breaking change: NO. A data directory whose WAL ends in a record this release
  cannot decode no longer starts (it lost that record before); every other WAL
  starts as before. `retention_secs: 0` now behaves as documented.
- User benefit: no acknowledged WAL record and no stream event kept "forever"
  is deleted silently at startup or by compaction.
