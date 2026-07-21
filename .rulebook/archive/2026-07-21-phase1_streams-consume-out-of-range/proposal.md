# Proposal: phase1_streams-consume-out-of-range

Source: docs/analysis/synap-audit/findings.md (M-012, residual)

## Why

The stream audit items are almost entirely fixed in the current code:
`Room::consume()` (`core/stream.rs:216`) already seeks in O(limit) instead of
scanning (M-016 done), retention protects unread events up to
`max_unread_buffer_size` and counts real losses (M-012 mostly done), and
time-based retention exists via `retention_secs`.

One residual correctness gap remains. When a consumer requests
`from_offset < min_offset` — i.e. the data it wants was already evicted — the
lookup at `core/stream.rs:221` does `from_offset.saturating_sub(self.min_offset)`,
which saturates to 0 and **silently** returns events starting from the earliest
still-buffered offset. The consumer receives a contiguous batch with no
indication that it skipped over evicted events, then advances its cursor as if
nothing was lost. This is a silent data gap — the exact failure Kafka avoids by
returning `OFFSET_OUT_OF_RANGE`.

## What Changes

- Add a `SynapError::OffsetOutOfRange { requested: u64, earliest: u64 }` variant
  in `core/error.rs`, mapped to HTTP 400 (mirroring `IndexOutOfRange`).
- Change `Room::consume()` to return `Result<Vec<StreamEvent>, SynapError>`:
  when `from_offset < self.min_offset` **and** `min_offset > 0` (data was
  genuinely evicted, not just an empty/new room), return
  `Err(OffsetOutOfRange { requested: from_offset, earliest: min_offset })`
  instead of silently truncating. A request at or above `min_offset`, or against
  a room that has never evicted, behaves exactly as today.
- Thread the new `Result` through the stream store's public `consume` and the
  server handler (`server/handlers/stream.rs`), returning the 400 with the
  earliest available offset so the client can re-seek deliberately.

## Impact
- Affected specs: specs/streams/spec.md (this task)
- Affected code: `crates/synap-core/src/core/stream.rs`,
  `crates/synap-core/src/core/error.rs`,
  `crates/synap-server/src/server/handlers/stream.rs`
- Breaking change: NO on the wire for in-range consumers; out-of-range consumers
  that previously got a silent partial batch now get an explicit 400 (this is the
  intended fix, not a regression).
- User benefit: a lagging consumer learns it fell off the retention window and
  can re-seek, instead of silently skipping committed events.
