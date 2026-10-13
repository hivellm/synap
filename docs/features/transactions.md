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

## Durability & replication (audit M-010, phase6k)

A committed `EXEC` is now durable and replicated, not just applied in memory:

- **`exec` returns the committed effects.** `TransactionManager::exec` returns
  `(results, writes)` where `writes` is the list of durable
  [`CommittedWrite`]s the transaction produced. Non-deterministic commands are
  resolved to their concrete effect — an `INCR` is recorded as the resulting
  `SET`, exactly as the non-transactional `INCR` handler logs — so replicas and
  the WAL never diverge from the master.
- **One shared persistence hook.** Every `EXEC` entry point (REST, RESP3,
  SynapRPC, admin, MCP) calls `PersistenceLayer::log_transaction(&writes)`, which
  maps each `CommittedWrite` to a persistence `Operation`, propagates all of them
  to replicas, and appends them to the WAL.
- **Logged as one atomic batch.** `log_transaction` uses a WAL batch API
  (`AsyncWAL::append_batch`) so the transaction's operations are written
  contiguously and confirmed as a unit under a single group-commit fsync — a
  committed `EXEC` survives a crash all-or-nothing rather than as interleavable
  single appends. With sync durability (`wal.fsync_mode: always`) the commit is
  fsynced before `EXEC` acknowledges success.
- **Replicated even without persistence.** Propagation is decoupled from the WAL
  (phase6j), so a master replicates a transaction to its replicas even when
  `persistence.enabled = false`.

### When the commit cannot be logged

If the WAL cannot log the transaction (a write, flush or fsync fails, or the
WAL is already poisoned by an earlier failure), every `EXEC` entry point
returns an error instead of a success reply:

| Transport | Error |
|-----------|-------|
| REST `POST /transaction/exec` | HTTP 500, `Internal error: transaction applied in memory but could not be logged to the WAL, so the commit is not durable ...` |
| StreamableHTTP `transaction.exec` | same message in the error envelope |
| MCP `synap_transaction_exec` | MCP internal error with the same message |
| RESP3 `EXEC` | `-ERR transaction applied in memory but could not be logged to the WAL, so the commit is not durable ...` |
| SynapRPC `EXEC` | `ERR ...` with the same message |

The transaction's effects were already applied in memory when logging failed,
and they are **not rolled back**: other clients can read them until the server
restarts, and a restart recovers only what reached the WAL. Treat the error as
"committed in memory, not durable". A failed flush or fsync also poisons the
WAL, so every later WAL append fails until the server restarts (see
[Persistence: write failures](../users/configuration/PERSISTENCE.md#write-failures-and-the-poisoned-wal)).

## Isolation (audit M-010, phase6k)

Beyond serializing transactions against each other, an `EXEC` is now isolated
from **non-transactional** writers to the same keys via a sharded per-key lock
manager (`KeyLockManager`) shared between the `KVStore` and the
`TransactionManager`:

- `exec` acquires the locks for the union of the keys it touches (in a
  deadlock-free order) and holds them across all of its commands.
- Single-key KV writes (`SET`, `DEL`, `INCR`/`DECR`, `GETSET`) take the same
  per-key lock, so a plain `SET k` issued while an `EXEC` touching `k` is running
  is ordered **entirely before or after** the transaction — never interleaved
  between its commands.
- Inside `exec`, the KV commands call the `*_unlocked` store methods because the
  transaction already holds the locks (avoiding re-entrant deadlock).

Scope note: isolation of non-transactional writers currently covers the KV
keyspace (the write path plain clients use for `SET`/`DEL`/`INCR`). Collection
commands inside an `EXEC` run under the serialized EXEC lock and each store
operation is internally atomic.
