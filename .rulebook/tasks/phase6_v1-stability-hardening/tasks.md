## 1. Implementation
- [ ] 1.1 Flake audit: run full suite ≥10x, log flaky tests with failure signatures
- [ ] 1.2 Replace sleep-based sync in flaky tests with Notify/barriers/polling-with-deadline
- [ ] 1.3 Re-enable or justify every #[ignore] test
- [ ] 1.4 Triage unwrap/expect/panic! in synap-core non-test code → ? + SynapError, or documented invariant
- [ ] 1.5 Triage unwrap/expect/panic! in synap-server non-test code (same rules)
- [ ] 1.6 Fix Resp3Config::enabled serde/Default contradiction + regression tests for YAML and struct paths
- [ ] 1.7 Unify listener bind defaults to loopback; document explicit exposure in config.example.yml
- [ ] 1.8 Resolve all 13 outstanding code markers in synap-server/src (implement the missing behavior or open a tracked issue and delete the marker)

## 2. Tail (mandatory — enforced by rulebook v5.3.0)
- [ ] 2.1 Update or create documentation covering the implementation (config docs + CHANGELOG bind-default note)
- [ ] 2.2 Write tests covering the new behavior
- [ ] 2.3 Run tests and confirm they pass
