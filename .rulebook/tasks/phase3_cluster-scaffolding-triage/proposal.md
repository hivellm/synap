# Proposal: phase3_cluster-scaffolding-triage

## Why

`crates/synap-core/src/cluster/` (`raft.rs`, `failover.rs`, `discovery.rs`,
`migration.rs`) is written but **not wired into the running server**. The server
`main.rs`/`lib.rs` never construct a `RaftNode`, `FailoverManager`,
`DiscoveryService` or migration engine (the only `Discovery*` matches in the
server are UMICP's unrelated `SynapDiscoveryService`). The modules survive
compilation only because they are blanketed in `#[allow(dead_code)]` — 9 in
`raft.rs`, 5 in `discovery.rs`, 4 in `migration.rs`, 3 in `failover.rs`. This is
~21 of the 43 `dead_code` suppressions in the whole workspace.

This is the worst of both worlds: the code carries a maintenance and
compile-time cost and implies a clustering/consensus capability that the shipped
server does not actually provide, while never being exercised by a single
non-test call path. Synap already has a **working** master-slave replication
path (`replication::MasterNode`, wired from `main.rs` and fed by
`persistence/layer.rs`), so high availability is not blocked on this Raft code.

A decision must be made and executed, not left implicit. Per the no-deferred and
research-first rules, the decision (an ADR) is the first work item of this task,
and its outcome is implemented in the same task — no orphan "TBD".

## What Changes

1. **Decide (ADR).** Write an ADR (`rulebook_decision_create`) evaluating the two
   viable options with effort and risk:
   - **(A) Gate as experimental** behind a non-default `cluster` cargo feature so
     the modules only compile when explicitly opted in, removing the blanket
     `dead_code` allows from the default build and making the "not production"
     status explicit in the type system rather than a comment.
   - **(B) Remove** the unwired modules entirely, relying on the existing
     master-slave replication for HA until a real consensus effort is scheduled.
   Fully wiring Raft into a correct, tested consensus layer is out of scope for a
   cleanup task (multi-week effort) and is explicitly **not** an option here.
2. **Execute the accepted option.** The recommended default is **(A)** — it
   preserves the team's investment, is non-destructive (honors the "never discard
   work" rule), and still removes the dead-code debt from the shipping build. If
   the ADR instead accepts (B), the removal is done with the modules' git history
   preserved and referenced from the ADR.
3. Either way, the default `cargo build`/`clippy`/test surface must no longer
   carry the cluster `dead_code` allows.

## Impact
- Affected specs: none (no runtime behavior change on the default build)
- Affected code: `crates/synap-core/src/cluster/*`, `crates/synap-core/src/lib.rs`
  (feature-gate the `cluster` module or remove it), `Cargo.toml`
  (new optional `cluster` feature if (A))
- Breaking change: NO on the default build (cluster is not reachable today)
- User benefit: an honest build surface — no phantom "cluster support", less
  dead code, and a recorded decision the next contributor can act on
