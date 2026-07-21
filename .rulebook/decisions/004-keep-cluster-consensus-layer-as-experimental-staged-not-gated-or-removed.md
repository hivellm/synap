# 4. Keep the cluster consensus layer as experimental/staged — not gated or removed

**Status**: accepted
**Date**: 2026-07-21
**Related Tasks**: phase3_cluster-scaffolding-triage

## Context

The `synap-v1-release` analysis flagged `crates/synap-core/src/cluster/`
(`raft.rs`, `failover.rs`, `discovery.rs`, `migration.rs`) as "unwired scaffolding
blanketed in `#[allow(dead_code)]`" and proposed either feature-gating or removing
it. phase3's item 1.2 required verifying that premise against the current source
before acting. That verification changed the picture:

- **Cluster mode is partially LIVE, not dead.** `hash_slot`, `topology`, `types`,
  `config` carry **zero** `dead_code` allows and are used by the running server:
  `KVStore` holds `cluster_topology`/`cluster_migration` and routes via
  `hash_slot`; `synap-server` `config.rs` embeds `ClusterConfig`; `main.rs`
  constructs `ClusterTopology`/`SlotMigrationManager`; `server/handlers/cluster.rs`
  serves the CLUSTER HTTP commands. `migration` is live too (SlotMigrationManager
  is constructed), though it carries 4 `dead_code` allows on not-yet-reached helpers.
- **Only the consensus/HA layer is unwired:** `raft.rs` (9 allows), `failover.rs`
  (3), `discovery.rs` (5). No non-test production path constructs `RaftNode`,
  `ClusterFailover` or `ClusterDiscovery` — the only two grep hits
  (`replication::failover::FailoverManager`, `umicp::discovery::SynapDiscoveryService`)
  are unrelated same-named modules.
- **It is not junk — it is tested.** `crates/synap-server/tests/cluster_integration_tests.rs`
  constructs `RaftNode` and `ClusterFailover` and exercises them; the layer is
  deliberate work materialized by the archived `phase10_cluster-consensus-failover-migration`
  and `phase11_cluster-raft-peer-networking` tasks. The `dead_code` allows sit on
  internal helpers that only become reachable once the layer is wired into `main.rs`.
- **Dependency direction is clean:** live modules do not reference raft/failover/
  discovery types (only comments and `u64` config values); `discovery` depends on
  the live `topology` (one-directional).

## Decision

**Keep the consensus layer in the default build, document it explicitly as
experimental/unwired, and treat its `#[allow(dead_code)]` markers as justified
staging artifacts — do not feature-gate and do not remove it.**

Add a module-level "Status: experimental" note to `raft.rs`, `failover.rs` and
`discovery.rs` stating that they are exercised only by
`cluster_integration_tests.rs` and are not yet constructed by the running server,
and pointing here.

## Alternatives Considered

- **Feature-gate raft/failover/discovery behind an off-by-default
  `cluster-consensus` cargo feature.** Rejected: it would drop
  `cluster_integration_tests.rs` from the default `cargo test`, *reducing* coverage
  on deliberate, passing code, and it adds cross-crate feature plumbing (synap-core
  → synap-server → CI) — real complexity whose only payoff is removing 17
  `dead_code` allows from the default lib build. Poor trade for a staged feature
  that is not rotting.
- **Remove raft/failover/discovery.** Rejected: it discards tested, intentional
  work (archived phase10/phase11) and deletes passing integration tests, violating
  the "never discard work" rule. Reintroducing a consensus layer later would cost
  far more than the allows save.

## Consequences

+ No churn, no coverage loss, no cross-crate feature plumbing.
+ The experimental/unwired status becomes explicit in the module docs instead of an
  implicit `dead_code` blanket, so the next contributor knows *why* the allows exist.
+ Corrects the analysis's over-broad "dead scaffolding" framing with file-level
  evidence.
- The consensus code and its 17 `dead_code` allows remain in the default build.
  Revisit (and consider gating) if/when the layer is either wired into `main.rs`
  (allows disappear naturally) or explicitly dropped from the roadmap.
