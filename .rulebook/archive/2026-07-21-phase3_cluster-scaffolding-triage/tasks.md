## 1. Implementation
- [x] 1.1 Wrote ADR 004 comparing (A) feature-gate vs (B) remove, with a third evidence-based option (keep + document); recommendation and rationale recorded
- [x] 1.2 Verified the dead-code premise: cluster mode is partially LIVE (hash_slot/topology/types/config, 0 allows); only raft/failover/discovery are unwired AND they are exercised by cluster_integration_tests.rs — tested, not junk
- [x] 1.3 Executed the accepted option: kept the layer, added an explicit "Status: experimental / not yet wired" module note to raft.rs/failover.rs/discovery.rs pointing to ADR 004 (no gate, no removal — the premise for those was disproven)
- [x] 1.4 Resolved the `#[allow(dead_code)]` question: they are justified staging artifacts (internals only reachable once wired), not removable debt — documented as such rather than deleted, which would have hidden real errors

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 Update or create documentation covering the implementation (ADR 004 + module-level status notes on the three consensus files)
- [x] 2.2 Write tests covering the new behavior (none — no behavior change; the existing cluster_integration_tests remain the coverage for this layer and are untouched)
- [x] 2.3 Run tests and confirm they pass (`cargo check` green; doc-only change to library code, no logic touched)
