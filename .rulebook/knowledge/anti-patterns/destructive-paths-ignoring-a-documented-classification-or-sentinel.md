# Destructive paths ignoring a documented classification or sentinel

**Category**: persistence
**Tags**: persistence, wal, streams, retention, data-loss, phase34

## Description

Two data-loss bugs had the same shape: the meaning was written down, but the code that deletes data never checked it. (1) phase25 typed the WAL scan's tail (`TailKind`: Incomplete, ChecksumMismatch, Blank, Unreadable) and documented that a crash never leaves an Unreadable record, yet `wal_frame::repair`, the only code that truncates the WAL, still truncated every `ScanEnd::TornTail` with `..` in its pattern, so startup silently deleted acknowledged records a newer release wrote. (2) `StreamConfig::retention_secs` documented `0 = infinite`, but `Room::compact` computed `now - 0` and dropped every event. Rules: (1) when you add a classification, grep every consumer that destroys data and make it match on the new field (no `..` over it); (2) a sentinel value documented on a config field must be special-cased at each read site, with a test that uses the sentinel against old data; (3) when a removal path updates derived state (here a room's `min_offset` and stats), test the case where it removes everything, and remove only a prefix of an offset-indexed buffer so contiguity holds even when timestamps step back. Fixed in phase34 (`PersistenceError::WALUnreadableRecord`, `Room::compact`).
