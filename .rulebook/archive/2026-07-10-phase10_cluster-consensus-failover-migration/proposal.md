# Proposal: phase10_cluster-consensus-failover-migration

Source: GitHub issue #233 (epic; deferred out of phase6 v1.0 hardening)

## Why
The cluster module (disabled by default) has core consensus mechanics left
unimplemented, so cluster mode cannot elect a leader, detect node failure, promote
a replica, or safely roll back a migration:
- `cluster/config.rs`: load cluster config from env/file
- `cluster/raft.rs`: request votes, send heartbeats (leader election + liveness)
- `cluster/failover.rs`: failure detection + replica promotion
- `cluster/migration.rs`: migration rollback (restore keys on abort)
This is the epic that finishes the cluster feature. It depends on cluster
initialization (#232).

## What Changes
1. `config.rs`: implement cluster config load from env/file into the topology.
2. `raft.rs`: implement RequestVote (grant/deny by term + log) and AppendEntries
   heartbeats; a follower with an expired election timeout becomes candidate and
   requests votes; a leader sends periodic heartbeats.
3. `failover.rs`: detect a dead node via missed heartbeats and promote the
   most-caught-up replica for its slots.
4. `migration.rs`: on migration abort, restore the moved keys to the source node
   (rollback) so a failed migration leaves no data loss.

## Impact
- Affected specs: cluster consensus + failover + migration (ADDED)
- Affected code: crates/synap-core/src/cluster/{config,raft,failover,migration}.rs
- Breaking change: NO (cluster disabled by default)
- User benefit: a usable clustered deployment with leader election, automatic
  failover, and safe slot migration
