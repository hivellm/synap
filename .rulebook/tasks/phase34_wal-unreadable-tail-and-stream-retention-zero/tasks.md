## 1. Implementation
- [x] 1.1 Refuse to start, leaving the file unchanged, when opening the WAL finds that it ends in a complete, checksum-valid record this release cannot decode (`TailKind::Unreadable`), with an error naming the WAL path, the byte offset and that a newer release may have written it — before: both WAL opens (`WriteAheadLog::open` in `recover_all`, `AsyncWAL::open` in `PersistenceLayer::new`) call `wal_frame::prepare` → `repair`, which truncated every `ScanEnd::TornTail` whatever its `kind` with only a warning; now `repair` returns the new `PersistenceError::WALUnreadableRecord { path, position, reason }` for a kind that is not `is_torn()` before touching the file, `refuses_startup` includes it, so `main.rs` exits with it (an unreadable frame followed by a valid one was, and stays, interior corruption)
- [x] 1.2 Keep truncating the tails a crash leaves (`Incomplete`, `ChecksumMismatch`, `Blank`) and starting, and test every shape through recovery and the WAL open — `repair` still truncates those three kinds; `torn_tail_then_new_writes_survive_two_restarts` now also covers a checksum mismatch and zero fill (two restarts each), next to the four cut-short shapes
- [ ] 1.3 Make `retention_secs: 0` mean no time-based retention for stream rooms wherever it is read, keeping the buffer caps and positive retention unchanged
- [ ] 1.4 Keep a room's earliest offset and statistics consistent when compaction removes events, including when it removes every event

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
