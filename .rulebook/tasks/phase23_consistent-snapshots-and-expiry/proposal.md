# Proposal: phase23_consistent-snapshots-and-expiry

Source: docs/analysis/next-release-review-2026-10-12/ (findings D-04 and D-07, priority P1, plus the same-key ordering part of D-08)
Target release: 1.4.0

## Why

- **D-04**: `maybe_snapshot` (`persistence/layer.rs`) reads the next WAL
  offset and only then dumps the live stores (`persistence/snapshot.rs`).
  Recovery replays from that offset inclusively (`persistence/recovery.rs`).
  A non-idempotent operation (LPUSH, HINCRBY) applied in memory before its
  store is dumped but logged at or after the cutoff ends up in both the
  snapshot and the replay, so it is applied twice. Dumping store by store also
  yields an image taken at mixed times.
- **Same-key ordering (D-08)**: handlers mutate memory first and log afterwards
  in a separate step, so two concurrent writers to one key can reach the WAL
  in the opposite order from memory. Replaying resulting-value records (INCR
  as SET, APPEND, SETRANGE) then ends on the wrong value.
- **D-07**: `KVStore::dump` (`synap-core/src/core/kv_store/store.rs`) drops
  expiry and snapshot restore sets keys with no TTL, so a key with a finite
  lifetime becomes permanent after a snapshot restart. WAL records hold the
  original relative TTL, and replay ignores the entry timestamp, so the TTL
  restarts from boot time and keys that expired during downtime come back.

## What Changes

- A design.md picks how a snapshot's state image is paired with its WAL
  position and how same-key WAL order follows memory order. Options include
  assigning the WAL sequence under the shard lock, or a commit barrier the
  snapshot takes briefly to capture a stable image before serializing it
  outside the barrier.
- Every operation is either in the snapshot image or after its WAL cutoff,
  never both.
- Concurrent writes to the same key reach the WAL in memory order.
- Absolute expiry (Unix milliseconds) is stored in snapshot format v4 and in
  WAL records that set or change expiry. v3 snapshots and legacy relative-TTL
  records still load. A legacy record's absolute expiry is its entry timestamp
  plus its TTL.
- Recovery omits keys whose absolute expiry has passed.
- Replication carries absolute expiry.

## Impact

- Affected specs: persistence (snapshot consistency, expiry)
- Affected code: `crates/synap-server/src/persistence/` (layer, snapshot,
  recovery, apply, types), `crates/synap-core/src/core/kv_store/`, the other
  core stores' dump paths, `crates/synap-server/src/replication/`
- Dependencies: phase20, phase21 (single mutation path to order), phase22 (WAL
  format version)
- Breaking change: YES for downgrades. Snapshot v4 cannot be read by 1.3.x.
  1.4.0 reads v3.
- User benefit: restarts reproduce exactly the state the server acknowledged,
  including how long each key has left to live.
