# Proposal: phase25_migrator-server-format-parity

Source: docs/analysis/next-release-review-2026-10-12/ (finding E-01, priority P1)
Target release: 1.4.0

## Why

`synap-migrate` cannot read the data a current server writes. It only looks for
`snapshot-v2-*` files (`crates/synap-migrate/src/snapshot.rs`) while the server
writes `snapshot-v3-*` (`crates/synap-server/src/persistence/snapshot.rs`). It
decodes integers as little-endian where the server writes big-endian. Its queue
and stream records differ from the server's, and its output lacks the checksum
the server requires. Selecting an older file would not help, because the codec
itself is different. Standalone-to-Hub migration is therefore broken, and the
migrator's own round-trip tests cannot catch it since they only test the
migrator against itself.

## What Changes

- The server's snapshot and WAL codecs live in one module that both the server
  and `synap-migrate` use. There is no second implementation.
- The migrator reads every snapshot version and WAL format the server reads
  (v3 and the phase23 v4 snapshots; legacy and phase22 WALs) and applies the
  WAL tail written after the snapshot before migrating.
- Every persisted structure migrates, with queue and stream metadata and
  absolute expiry, scoped for Hub mode.
- The migrator writes only to a new location. It validates the whole input
  first and rejects corrupt or unsupported data before writing anything, so
  the original directory is the rollback.
- Fixtures come from real 1.3.2 and 1.4.0 servers.

## Impact

- Affected specs: migrate
- Affected code: `crates/synap-migrate/`, `crates/synap-server/src/persistence/`
  (codec extraction), possibly `crates/synap-core/` as the shared home of the
  codec
- Dependencies: phase18 (Hub scoping), phase22 (WAL format), phase23 (snapshot
  v4), phase24 (queue metadata)
- Breaking change: NO for users. The migrator's v2-only reader goes away.
- User benefit: a standalone server's data can actually be moved into Hub mode.
