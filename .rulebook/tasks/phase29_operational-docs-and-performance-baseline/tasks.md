## 1. Implementation
- [ ] 1.1 Add a benchmark script recording p50/p95/p99 latency, throughput, memory, workload, durability mode, build and hardware per run
- [ ] 1.2 Add a scheduled and manually triggered benchmark workflow for a controlled runner, with no latency gate on shared runners
- [ ] 1.3 Record the 1.4.0 baseline and a 1.3.2 comparison from repeated runs
- [ ] 1.4 Write the 1.4.0 guarantees document: durability per fsync mode, authorization per transport, Hub mode, limits, and error replies versus in-memory state
- [ ] 1.5 Label historical analyses under `docs/analysis/` with the revision they describe and link the current architecture

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
