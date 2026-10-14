# One durable function per command, mirroring the store signature

**Category**: persistence
**Tags**: persistence, wal, replication, transport-parity, phase21

## Description

To make every transport persist and replicate writes, put one function per mutating store method in persistence::durable, taking (log: Option<&PersistenceLayer>, &store, <the store method's own args>) and returning the store's result. It applies the mutation, then records the resulting effect (INCR/APPEND/SETRANGE as value + remaining expiry read under the shard lock, pops as the members removed, HINCRBY/ZINCRBY as the resulting field/score) and maps a logging failure to an error. With log == None it is exactly the store call, so the no-persistence hot path pays nothing. Because the signature mirrors the store, routing ~150 call sites was a mechanical rewrite (state.X_store.m(args) -> durable::m(state.persistence.as_deref(), &state.X_store, args).await), done with a paren-matching script; sorted-set methods that return plain values needed a `?`/match. A parity test that fingerprints every store, waits for a live replica to match and compares a WAL-only recovery caught an envelope encoding bug the replies never showed.

## When to Use

A cross-cutting effect (persistence, replication, notifications) that every transport must apply to the same store mutations.

## When NOT to Use

Read paths, or concerns already enforced inside the core store itself.
