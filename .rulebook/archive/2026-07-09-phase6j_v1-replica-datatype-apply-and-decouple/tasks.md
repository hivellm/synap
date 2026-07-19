## 1. Implementation
- [x] 1.1 Extend ReplicaNode to hold hash/list/set/sorted-set/queue stores
- [x] 1.2 Apply every Operation variant in ReplicaNode::apply_operation (mirror recovery.rs)
- [x] 1.3 Decouple replication propagation from the persistence log (shared propagate hook)
- [x] 1.4 Surface replication status (role, connected replicas, lag) in INFO/metrics
- [x] 1.5 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (replication.md)
- [x] 2.2 Write tests covering the new behavior (master-replica convergence for every datatype; reconnect resync)
- [x] 2.3 Run tests and confirm they pass
