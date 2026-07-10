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
