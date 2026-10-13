# Proposal: phase33_blocking-pop-multi-key-wakeup

Source: found during phase23, see
`.rulebook/archive/2026-10-13-phase23_consistent-snapshots-and-expiry/design.md` §4
Target release: 1.4.0

## Why

A client blocked on several keys must wake as soon as any of them receives
data. Multi-key BLPOP, BRPOP, BZPOPMIN and BZPOPMAX (`ListStore` and
`SortedSetStore` in `synap-core`, including the phase23 `*_guarded` variants
that `persistence::durable` uses) subscribe one broadcast receiver per key and
then wait on them in turn (`for rx in receivers { rx.recv().await }`). A push
to the second key does not wake the client until the first key is pushed too,
or the timeout expires. The receivers are also created only after the first
pop attempt, so a push that lands between "every key was empty" and
"subscribed" is lost until a later push, and the waiter that a push wakes is
whoever reaches the shard lock first rather than the client that has waited
longest. Redis wakes a blocked client on the first listed key that receives
data, pops from the first non-empty key in argument order, and serves clients
blocked on one key first-come first-served.

## What Changes

- A shared blocking-waiter registry in `synap-core`: every blocking pop
  registers one waiter on all of its keys before its last emptiness check, so
  a push to any listed key wakes it and no push between the check and the
  wait is missed. Pushes wake waiters in the order they blocked (FIFO per
  key). A waiter that leaves (pops, times out or is cancelled) hands any
  wake-up it can no longer use to the next waiter on each of its keys that
  still holds data.
- `ListStore` (BLPOP, BRPOP, BRPOPLPUSH) and `SortedSetStore` (BZPOPMIN,
  BZPOPMAX) use the registry instead of per-key broadcast channels, in both the
  plain and the `*_guarded` variants. The guard (the persistence ordering
  stripes) is still held only while popping, never while waiting.
  BRPOPLPUSH stays single-source.
- Sorted-set writes that add members without ZADD (ZINCRBY, ZINTERSTORE,
  ZUNIONSTORE, ZDIFFSTORE) also wake blocked BZPOPMIN/BZPOPMAX clients.
- Timeout semantics are unchanged on every transport.

## Impact

- Affected specs: lists (blocking pops)
- Affected code: `crates/synap-core/src/core/` (new blocking-waiter module,
  `list/store.rs`, `sorted_set/store.rs`), tests in `synap-core` and a new
  `synap-server` integration test
- Breaking change: NO
- User benefit: BLPOP/BRPOP/BZPOPMIN/BZPOPMAX on several keys return as soon
  as any key receives data, on RESP3 and SynapRPC, with or without
  persistence, and clients blocked on one key are served in the order they
  blocked.
