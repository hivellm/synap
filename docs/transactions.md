# Transactions (MULTI / EXEC / WATCH)

Synap supports Redis-style transactions: `MULTI` opens a transaction, commands are
queued, `EXEC` runs them, `DISCARD` cancels, and `WATCH` provides optimistic
concurrency (EXEC aborts if a watched key changed).

## Atomicity (audit M-008)

`TransactionManager` holds an EXEC mutex across the entire critical section — the
WATCH check, command execution, and version bump — so:

- two transactions never interleave with each other, and
- the WATCH check-and-apply is one atomic step (no TOCTOU between the version
  check and execution).

Previously EXEC ran its commands with no lock and the WATCH check was separate
from execution, so concurrent EXECs could interleave.

## Current limitations (tracked in phase6k)

- **Durability / replication (M-010)**: transaction commands execute directly on
  the stores, so they are not yet written to the WAL or propagated to replicas —
  a committed EXEC can be lost on crash and is not replicated.
- **Isolation from plain writers**: the EXEC mutex serializes transactions against
  each other but not against non-transactional writes to the same keys; full
  isolation needs per-key locking.
