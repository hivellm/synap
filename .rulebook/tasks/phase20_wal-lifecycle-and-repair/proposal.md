# Proposal: phase20_wal-lifecycle-and-repair

Source: docs/analysis/next-release-review-2026-10-12/ (findings D-01, D-02 and D-06, priority P1)
Target release: 1.4.0

## Why

The async WAL writer (`crates/synap-server/src/persistence/wal_async.rs`) can
lose writes it has already acknowledged:

- **D-01**: the batch-timeout branch of `writer_loop` is only armed while the
  batch is non-empty, and the Periodic fsync deadline is only checked while
  processing writes. After a group whose deadline has not passed, the entries
  sit in the 64 KiB `BufWriter` until the next write arrives, which on an idle
  server may be never. When the channel closes the loop breaks and drops the
  `BufWriter`. Tokio's `BufWriter` does not flush on drop, so shutdown discards
  the buffer. `PersistenceLayer::flush` (`persistence/layer.rs`) is a no-op.
  Never mode leaves bytes in the application buffer instead of only skipping
  fsync.
- **D-02**: a failed group flush or fsync only logs a warning. The per-entry
  results, already `Ok`, are sent as success. The EXEC paths swallow
  `log_transaction` errors too, contradicting the fsync-before-success promise
  in `docs/features/transactions.md`.
- **D-06**: `scan_for_last_offset` stops at a torn entry but the file is opened
  in append mode, so new entries land after the garbage. On the next restart
  the parser reads misaligned frames and the writes made after the first crash
  are lost. The scanner reads each checksum but never verifies it, and trusts
  the frame length, so `vec![0u8; size]` on a corrupt length can request an
  enormous allocation and abort the process. The sync scanner in
  `persistence/wal.rs` behaves the same way.

## What Changes

- Periodic mode flushes and fsyncs on its own interval, idle or not. Never mode
  hands bytes to the OS after each group without fsync.
- An awaited flush barrier backs `PersistenceLayer::flush`, and graceful
  shutdown drains the WAL before exiting.
- A failed flush or fsync fails every acknowledgement in its group and poisons
  the writer: later appends fail until restart, because after a failed fsync
  the kernel may have dropped dirty pages and the file contents are unknown.
  The health endpoint reports the poisoned WAL.
- All five EXEC entry points (REST, admin, MCP, RESP3, SynapRPC) return an
  error when the transaction could not be logged. The error says the commit is
  not durable. The in-memory effect is not rolled back, and this is documented.
- Scanning verifies checksums and bounds frame length. An invalid frame with no
  complete frame after it is a torn tail: the file is truncated to the last
  valid frame before the writer opens. An invalid frame followed by a valid one
  is interior corruption: startup fails with the byte position.

## Impact

- Affected specs: persistence (WAL lifecycle)
- Affected code: `crates/synap-server/src/persistence/wal_async.rs`,
  `persistence/wal.rs`, `persistence/layer.rs`, the five EXEC handlers,
  shutdown in `main.rs`, the health handler
- Dependencies: none; precedes the WAL format work in phase22
- Breaking change: NO on the wire or on disk. Clients see errors where they
  previously got false success.
- User benefit: the configured fsync mode bounds data loss as documented, and
  a crash cannot make later writes unrecoverable.
