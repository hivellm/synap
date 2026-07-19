> **NOT IMPLEMENTED — closed by decision on 2026-07-19.** Cluster mode stays
> single-node: `raft.rs` elects itself leader immediately and sends no peer
> traffic, `failover.rs` detects nothing. That is the shipped behavior and it is
> deliberate — cluster is `enabled: false` by default, so no deployment depends
> on the missing half. None of the items below were done; the task is archived
> to stop it reading as pending work rather than because it was finished.
>
> The gap remains tracked by **hivellm/synap#233**, which the code still points
> at from six `tracked in hivellm/synap#233` markers in `raft.rs`,
> `failover.rs` and `migration.rs`. Reopen from that issue if multi-node
> cluster is ever picked up; do not resurrect this task.

## 1. Implementation
- [ ] 1.1 Raft peer RPC transport (RequestVote/AppendEntries, length-prefixed bincode over TCP)
- [ ] 1.2 Election: send RequestVote to peers, tally majority, become leader only on majority
- [ ] 1.3 Leader sends periodic AppendEntries heartbeats to all peers
- [ ] 1.4 Failover: detect dead node via missed heartbeats + promote most-caught-up replica
- [ ] 1.5 Wire peer addresses from config into the Raft node
- [ ] 1.6 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (cluster.md)
- [ ] 2.2 Write tests covering the new behavior (3-node single-leader election; re-election on leader loss)
- [ ] 2.3 Run tests and confirm they pass

<!-- tail-waiver: Task closed as NOT IMPLEMENTED, not as completed — no code, docs or tests were produced, so the docs+tests tail has nothing to cover. Decision on 2026-07-19: multi-node cluster is out of scope; cluster mode ships single-node and is disabled by default, so no deployment depends on the missing peer networking. The gap stays tracked by hivellm/synap#233, which the source still references from six markers in raft.rs, failover.rs and migration.rs. -->
