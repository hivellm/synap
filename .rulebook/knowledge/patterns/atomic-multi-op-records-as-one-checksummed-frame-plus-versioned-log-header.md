# Atomic multi-op records: one checksummed frame, plus a versioned log header

**Category**: persistence
**Tags**: persistence, wal, transactions, replication, versioning, phase22

## Description

"Write the batch contiguously in one group commit" is not crash atomicity:
each operation still had its own frame and checksum, so torn-tail repair kept
every complete frame before the tear and recovery exposed a prefix of the
transaction. Make the unit of atomicity the unit of checksumming: wrap the
operations in ONE record (`Operation::Transaction { operations }`) so the
existing torn-tail repair discards it whole, and ship the same record as ONE
replication command so a replica that reads whole length-prefixed commands
applies it whole or not at all. One record also means one WAL offset and one
replication offset, so a resync resumes at the transaction, not inside it.

Adding a record kind old binaries cannot decode needs a format version. Put a
magic + version header at byte 0 of new files; detect headerless legacy files
by the magic (a legacy file starts with a frame length, which can never equal
the magic), read them as version 1, and upgrade them before the first append
via temp file (header + unchanged frames) + fsync + rename, so a crash leaves
either file intact. Refuse to start on a newer version, naming both versions.
Append new enum variants at the END so bincode variant indices, and old files,
stay valid.

Verify the legacy path with a fixture produced by an INDEPENDENT encoder of the
old format (a small script), not by the current Rust encoder, or the test only
proves the code agrees with itself.

## When to Use

Any log or stream whose consumers must see a multi-operation unit all or
nothing after a crash or a dropped connection; any on-disk format change that
old binaries cannot read.

## When NOT to Use

Single-operation writes (already one frame); formats rewritten atomically as a
whole (snapshots).
