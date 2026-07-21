# 5. Native-transport transactional writes via TXQUEUE wrapper command (client_id-keyed), not connection-scoped MULTI

**Status**: proposed
**Date**: 2026-07-21
**Related Tasks**: phase7_native-transport-transactions

## Context

Transactions are client_id-keyed on every wire: the HTTP command endpoint passes client_id in the payload, and the native MULTI/EXEC/DISCARD/WATCH/UNWATCH commands already take client_id as their first argument. But no native write command carries a client_id, so nothing could ever be queued into an open transaction from SynapRPC or RESP3 — and a mapped write sent between MULTI and EXEC executed immediately, silently bypassing the transaction.

## Decision

Add one wrapper command to both native dispatchers: TXQUEUE &lt;client_id&gt; &lt;CMD&gt; &lt;args...&gt;. The server parses the inner command (SET/DEL/INCR[BY]/DECR[BY]/HSET/HDEL/HINCRBY/LPUSH/RPUSH/LPOP/RPOP/SADD/SREM — exactly the TransactionCommand enum) and queues it via TransactionManager::queue_command_if_transaction, replying QUEUED; an unknown inner command or a missing open transaction is an explicit error. SDK command maps wrap their normal native mapping in TXQUEUE whenever the payload carries a client_id, and refuse (UnsupportedCommandError) commands that are not queueable.

## Alternatives Considered

- Connection-scoped Redis semantics (MULTI binds to the TCP connection, subsequent commands queue implicitly): most idiomatic for RESP3, but requires plumbing connection identity through both stateless dispatchers and Thunder's Dispatch trait, and breaks cross-transport transactions (begin over HTTP, queue over RPC) that client_id keying gives for free. Can still be layered on later without breaking TXQUEUE.
- Optional trailing client_id argument on every write command: pollutes every command's arity and is ambiguous with legitimate trailing arguments (e.g. SET key value EX ttl).

## Consequences

Transactional writes work identically on HTTP, SynapRPC and RESP3, keyed by the same client_id — a transaction can even span transports. Native wires stay stateless per-connection. Redis clients using bare MULTI over RESP3 still get Synap's client_id-flavored semantics (documented); true connection-scoped emulation remains possible as a future additive layer.
