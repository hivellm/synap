# Proposal: phase6k_v1-transaction-durability-and-per-key-isolation

Source: docs/analysis/synap-audit/ (M-010 + M-008 refinement); follow-up of phase6d

## Why
phase6d closed the core of M-008: EXEC is now serialized under an EXEC mutex so
two transactions cannot interleave and the WATCH check-and-apply is atomic. Two
gaps remain. (1) M-010: transaction commands still execute directly on the stores,
bypassing the WAL/persistence log and (therefore, via the phase6e piggyback)
replication — a committed EXEC can be lost on crash and is never replicated.
(2) The EXEC mutex serializes transactions against each other but not against
non-transactional concurrent writers; true isolation needs per-key locking.

## What Changes
1. Make `TransactionManager::exec` expose the list of executed commands so the
   caller can persist them (the manager lives in `synap-core` and cannot call the
   `synap-server` persistence layer directly — the DAG forbids core→server).
2. Add a shared `synap-server` helper that maps each executed `TransactionCommand`
   to a persistence `Operation` and logs it (which also replicates via the phase6e
   piggyback), and call it from all five EXEC entry points (RESP3, SynapRPC,
   admin, kv, mcp). Deterministic ops (e.g. INCR/POP) log their resulting effect,
   not the request, to keep replicas/WAL consistent.
3. Add a WAL batch API so an EXEC is logged as one atomic unit.
4. Introduce per-key locking so an EXEC is isolated from non-transactional writers.

## Impact
- Affected specs: transaction durability + isolation (ADDED/MODIFIED)
- Affected code: `crates/synap-core/src/core/transaction.rs`, the five EXEC
  handlers in `synap-server`, `persistence` (batch API)
- Breaking change: NO
- User benefit: MULTI/EXEC becomes durable, replicated, and fully isolated —
  matching Redis semantics
