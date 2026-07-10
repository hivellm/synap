# Proposal: phase10_cluster-config-topology-wiring

Source: GitHub issue #232 (deferred out of phase6 v1.0 hardening)

## Why
The cluster feature (disabled by default) is never actually initialized from
config: `main.rs` constructs `AppState` with `cluster_topology: None` and
`cluster_migration: None` unconditionally. Even with cluster config present, the
node runs as standalone because the topology and slot-migration manager are never
built. This is the entry point for the whole cluster feature — without it, the
KV store's cluster routing (`check_cluster_routing`) is dead.

## What Changes
1. Add a `cluster` section to `ServerConfig` (enabled flag, this node id/address,
   seed nodes, slot assignment) if not already present.
2. In `main.rs`, when `cluster.enabled`, build `ClusterTopology` and the
   `SlotMigrationManager` from config and pass them into `AppState` and the
   `KVStore` (via `new_with_cluster`), instead of `None`.
3. Surface cluster mode in startup logs and in `INFO cluster`.

## Impact
- Affected specs: cluster initialization (ADDED)
- Affected code: crates/synap-server/src/main.rs, config.rs,
  crates/synap-core/src/cluster/{topology,config}.rs
- Breaking change: NO (cluster stays disabled by default)
- User benefit: cluster mode can actually be enabled from config
