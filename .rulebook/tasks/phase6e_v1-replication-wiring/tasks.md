## 1. Implementation
- [ ] 1.1 Construct MasterNode or ReplicaNode from main.rs based on config.replication and store it in AppState
- [ ] 1.2 Spawn the master replica-listener / replica connector from main.rs
- [ ] 1.3 Call replicate()/log-append from every write handler (not just KV)
- [ ] 1.4 Extend replicated Operation coverage to Hash/List/Set/SortedSet/Queue/Stream
- [ ] 1.5 Verify replica applies the full operation set; validate PartialSync and FullSync end to end
- [ ] 1.6 Expose replication status (role, connected replicas, lag) in INFO/metrics
- [ ] 1.7 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (replication ops guide)
- [ ] 2.2 Write tests covering the new behavior (master-replica sync of every datatype; reconnect triggers correct resync)
- [ ] 2.3 Run tests and confirm they pass
