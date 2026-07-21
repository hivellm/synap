## 1. Implementation
- [ ] 1.1 Write an ADR (`rulebook_decision_create`) comparing (A) feature-gate as experimental vs (B) remove, with effort/risk and a recommendation
- [ ] 1.2 Confirm no non-test call path constructs the cluster types (`RaftNode`/`FailoverManager`/`DiscoveryService`/migration) — verify the dead-code premise before acting
- [ ] 1.3 Execute the accepted option: (A) introduce an optional `cluster` cargo feature and gate `mod cluster` behind it, OR (B) remove the unwired modules
- [ ] 1.4 Remove the now-unnecessary `#[allow(dead_code)]` suppressions from the default build

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation (record the decision in the ADR and note cluster status in README/AGENTS.override.md)
- [ ] 2.2 Write tests covering the new behavior (if (A): CI check that `--features cluster` still compiles; if (B): existing tests pass without the modules)
- [ ] 2.3 Run tests and confirm they pass (`cargo check` + `clippy -D warnings` default features + full suite)
