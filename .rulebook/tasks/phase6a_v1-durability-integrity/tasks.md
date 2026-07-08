## 1. Implementation
- [ ] 1.1 Add Hash/List/Set/SortedSet sections to the snapshot writer in snapshot.rs create_snapshot
- [ ] 1.2 Add symmetrical reads for those sections in snapshot.rs load_latest; bump SNAPSHOT_VERSION with backward-compatible old-format read
- [ ] 1.3 Populate HashStore/ListStore/SetStore/SortedSetStore from the snapshot in recovery.rs
- [ ] 1.4 Verify the CRC64 on load: recompute while streaming and return SnapshotCorrupted on mismatch
- [ ] 1.5 Resolve stream WAL duplication: replay StreamPublish in recovery.rs OR stop logging streams to WAL; document the chosen path
- [ ] 1.6 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (persistence format doc)
- [ ] 2.2 Write tests covering the new behavior (round-trip snapshot for every datatype; corrupt-checksum rejection; crash-after-snapshot recovery)
- [ ] 2.3 Run tests and confirm they pass
