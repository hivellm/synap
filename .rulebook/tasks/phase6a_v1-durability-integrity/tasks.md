## 1. Implementation
- [x] 1.1 Add Hash/List/Set/SortedSet sections to the snapshot writer in snapshot.rs create_snapshot
- [x] 1.2 Add symmetrical reads for those sections in snapshot.rs load_latest; bump SNAPSHOT_VERSION with backward-compatible old-format read
- [x] 1.3 Populate HashStore/ListStore/SetStore/SortedSetStore from the snapshot in recovery.rs
- [x] 1.4 Verify the CRC64 on load: recompute while streaming and return SnapshotCorrupted on mismatch
- [x] 1.5 Resolve stream WAL duplication: stop logging StreamPublish to the WAL (streams persist via StreamPersistence + snapshot)
- [x] 1.6 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/persistence-snapshot-format.md)
- [x] 2.2 Write tests covering the new behavior (round-trip of every datatype; corrupt-checksum rejection)
- [x] 2.3 Run tests and confirm they pass
