## 1. Implementation
- [ ] 1.1 cluster/config.rs: load cluster config from env/file into the topology
- [ ] 1.2 cluster/raft.rs: RequestVote (term/log-based grant) + candidate election on timeout
- [ ] 1.3 cluster/raft.rs: AppendEntries heartbeats (leader liveness) + follower term tracking
- [ ] 1.4 cluster/failover.rs: dead-node detection via missed heartbeats + replica promotion
- [ ] 1.5 cluster/migration.rs: migration rollback (restore keys to source on abort)
- [ ] 1.6 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (cluster.md)
- [ ] 2.2 Write tests covering the new behavior (vote grant/deny, election, failover, rollback)
- [ ] 2.3 Run tests and confirm they pass
