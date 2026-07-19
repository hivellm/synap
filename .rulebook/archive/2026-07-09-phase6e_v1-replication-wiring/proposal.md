# Proposal: phase6e_v1-replication-wiring

Source: docs/analysis/synap-audit/ (M-005; relates to M-010)

## Why
Replication is configured but never actually runs in the shipped server. `main.rs` imports
`NodeRole` and sets `config.replication.*` from the CLI (`main.rs:10,123-140`) but never
constructs `MasterNode`/`ReplicaNode`; `AppState` has no master/replica field
(`server/handlers/mod.rs` has no such field). `MasterNode::new`/`ReplicaNode::new` and
`master.replicate()` are only ever called from `#[cfg(test)]` code and `failover.rs`
(verified: no non-test call site in `src`). So starting with `--role master --replica-listen ...`
accepts the flags but spawns no replica listener and replicates no writes — a replica receives
nothing beyond an initial snapshot at best. For a 1.0 that advertises master-replica
replication, the feature must be wired into the running server and the live write path.
Additionally, the `replicate()` path only handles KV/partition operations, not
Hash/List/Set/SortedSet/Queue/Stream.

## What Changes
1. Construct the `MasterNode` (role=master) or `ReplicaNode` (role=replica) from `main.rs`
   when `config.replication.enabled`, store it in `AppState`, and spawn its listener/connector.
2. Call `replicate()` (or the replication-log append) from every write handler, so all writes
   feed the replication stream — not just KV. Extend the replicated `Operation` coverage to all
   datatypes (reuse the persistence `Operation` enum already covering them).
3. Verify the replica applies the full operation set on receipt and that partial resync
   (`PartialSync`) and full resync (`FullSync`) both work end to end against the wired path.
4. Expose replication status (role, connected replicas, replica lag) in INFO/metrics.

## Impact
- Affected specs: replication wiring + datatype coverage (ADDED)
- Affected code: `crates/synap-server/src/main.rs`, `server/handlers/mod.rs` (AppState),
  the write handlers, `replication/master.rs`, `replication/replica.rs`
- Breaking change: NO (feature was inert); enabling it changes runtime topology behavior
- User benefit: master-replica replication actually works — a replica stays in sync with the
  master across all datatypes, enabling read scaling and failover
