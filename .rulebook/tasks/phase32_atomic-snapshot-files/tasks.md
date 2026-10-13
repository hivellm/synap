## 1. Implementation
- [x] 1.1 Write snapshots to a temporary file, fsync it, rename it to the final name and fsync the directory; remove leftover temporary files before the next write
- [x] 1.2 Name snapshots with a sequence number so names never collide, and order loading by that sequence (old timestamp names load as older)
- [x] 1.3 Bound every length read from a snapshot by the bytes left in the file and report truncated or undecodable files as corrupted
- [x] 1.4 Recover from the newest valid snapshot, skipping damaged newer files only when the WAL still covers the older snapshot's cutoff, and refuse startup otherwise
- [x] 1.5 Fsync the WAL up to the cutoff before publishing a snapshot, serialize snapshots, and make the WAL continue at or above the recovered snapshot's cutoff
- [x] 1.6 Run retention only after the new snapshot is durable, never delete the snapshot just written, and treat removal failures as warnings

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [x] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
