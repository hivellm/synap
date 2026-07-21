# TXQUEUE wrapper for carrying cross-cutting context on flat wire protocols

**Category**: wire-protocol
**Tags**: transactions, txqueue, resp3, synaprpc, command-map

## Description

When a flat command wire (Redis-style RESP3 or positional-args RPC) needs to carry cross-cutting context that its commands have no slot for (here: the transaction client_id), wrap the inner command in a single new envelope command — TXQUEUE &lt;client_id&gt; &lt;CMD&gt; &lt;args...&gt; — instead of changing every command's arity or plumbing connection state through stateless dispatchers. SDK-side, implement it in the command map by recursion: strip the context key from the payload, map normally, then wrap if the resulting raw command is in the server's queueable allowlist; return null (explicit UnsupportedCommandError) otherwise so nothing executes with the context silently dropped. Keep the allowlist mirrored in server dispatchers and all SDK maps (it is exactly the TransactionCommand enum).

## When to Use

Adding per-request context (transactions, tracing, tenancy) to Redis-shaped or positional native protocols without breaking existing command arities.

## When NOT to Use

When the context is connection-scoped by nature and the dispatcher already has connection identity — then bind state to the connection instead.
