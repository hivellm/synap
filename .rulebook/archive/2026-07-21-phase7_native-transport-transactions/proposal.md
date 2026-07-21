# Proposal: phase7_native-transport-transactions

Source: follow-up from phase6_csharp-http-command-names (release validation 2026-07-21).

## Why

Transactions are only usable over StreamableHTTP today. The server's native
dispatchers (SynapRPC and RESP3) implement `MULTI`/`EXEC`/`DISCARD`/`WATCH`/
`UNWATCH` keyed by `client_id`, but no *write* command on those wires carries a
`client_id` — so nothing can ever be queued into an open transaction from a
native transport. Worse, a mapped write sent between MULTI and EXEC executes
immediately, silently bypassing the transaction. The Python SDK now refuses
such writes with `UnsupportedCommandError` (phase6); the TS SDK still has the
silent-bypass behavior, and C# maps transactions over native transports
without queueing semantics.

## What Changes

- Design and implement a queuing mechanism for transactional writes on the
  native wires. Candidate designs (decide in-task):
  a) a `QUEUED <client_id> <raw command...>` wrapper command, or
  b) connection-scoped transactions (Redis semantics — MULTI binds to the
     connection, subsequent commands on that connection queue implicitly), or
  c) optional trailing `client_id` argument on write commands.
  Redis-style connection-scoped (b) is the most idiomatic for RESP3.
- Wire the chosen mechanism into both native dispatchers and the
  TransactionManager.
- Update TS/Python/C# SDK command maps to use it; remove the Python
  client_id refusal guard once queuing works; align the TS SDK (which today
  silently bypasses).
- Parity tests: MULTI → queued write → EXEC/DISCARD round-trip on all three
  transports in every SDK's S2S suite.

## Impact
- Affected specs: transaction behavior on native wires (new spec section)
- Affected code: server RESP3 + SynapRPC dispatchers, TransactionManager,
  all three SDK command maps and their parity tests
- Breaking change: NO (adds capability; HTTP behavior unchanged)
- User benefit: transactions work identically on every transport, and no
  transport silently executes writes that the caller believes are queued
