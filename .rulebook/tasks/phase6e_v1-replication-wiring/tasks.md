## 1. Implementation
- [x] 1.1 Construct MasterNode / ReplicaNode from main.rs based on config.replication (self-running)
- [x] 1.2 Master replica-listener / replica connector spawn on construction
- [x] 1.3 Propagate every logged write to replicas via the persistence layer (all datatypes)
- [x] 1.4 Replicated Operation coverage includes Hash/List/Set/SortedSet/Queue/Stream (master side)
- [x] 1.5 Replica applies KV/stream now; full-datatype replica apply moved to phase6j
- [x] 1.6 Add MasterNode::replication_offset(); full INFO status moved to phase6j
- [x] 1.7 Gate: cargo check, clippy -D warnings, fmt --check

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [x] 2.1 Update or create documentation covering the implementation (docs/replication.md)
- [x] 2.2 Write tests covering the new behavior (logged write reaches the replication master)
- [x] 2.3 Run tests and confirm they pass
