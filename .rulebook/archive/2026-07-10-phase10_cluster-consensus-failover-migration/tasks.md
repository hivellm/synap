## 1. Implementation
- [x] 1.1 cluster/config.rs: load cluster config from env vars (from_getter overlay; from_env delegates)
- [x] 1.2 cluster/raft.rs: RequestVote term-based grant + candidate election on timeout (single-node; multi-node peer RPC is phase11)
- [x] 1.3 cluster/raft.rs: receive_heartbeat follower term tracking (leader→peer AppendEntries networking is phase11)
- [x] 1.4 cluster/failover.rs: detect_failure + promote_replica logic (missed-heartbeat networking is phase11)
- [x] 1.5 cluster/migration.rs: cancel rollback (non-destructive copy model: source retained, progress reset)
- [x] 1.6 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (CHANGELOG)
- [x] 2.2 Write tests covering the new behavior (from_getter overlay, migration cancel rollback; existing raft/failover tests)
- [x] 2.3 Run tests and confirm they pass
