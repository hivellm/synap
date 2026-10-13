# Proposal: phase22_atomic-transaction-records

Source: docs/analysis/next-release-review-2026-10-12/ (finding D-03, priority P1)
Target release: 1.4.0

## Why

`docs/features/transactions.md` promises that a committed EXEC survives a crash
all-or-nothing. It does not. `AsyncWAL::append_batch` writes each operation of
the transaction as an ordinary `WALEntry` with its own length and checksum
(`persistence/wal_async.rs`), and replay (`persistence/wal.rs`) accepts every
complete entry before a torn one. Nothing marks where a transaction ends, so a
crash between the first and second operation of an EXEC recovers the first
alone. Replication (`persistence/layer.rs`) sends the operations separately as
well, so a replica that disconnects mid-transaction can hold part of it.

## What Changes

- The WAL gets a format version. Headerless files are read as the legacy
  format, and a file with an unknown newer version fails startup with a clear
  error.
- A transaction is written as one framed record under one checksum. A torn
  transaction record is discarded whole by the phase20 torn-tail handling.
- Replay applies a transaction record all-or-nothing.
- Replication ships a transaction as one unit that a replica applies whole or
  not at all.
- Legacy WALs written by 1.3.x replay as before. A 1.3.x binary cannot read a
  1.4.0 WAL. The phase30 migration guide documents this and the backup step
  before upgrading.

## Impact

- Affected specs: persistence (transaction records)
- Affected code: `crates/synap-server/src/persistence/wal_async.rs`,
  `persistence/wal.rs`, `persistence/types.rs`, `persistence/layer.rs`,
  `persistence/apply.rs`, `crates/synap-server/src/replication/`
- Dependencies: phase20 (torn-tail handling and scanner validation)
- Breaking change: YES for downgrades. 1.4.0 reads 1.3.x WALs, but 1.3.x cannot
  read a WAL written by 1.4.0.
- User benefit: a crash never exposes half of a transaction, on the master or
  on a replica.
