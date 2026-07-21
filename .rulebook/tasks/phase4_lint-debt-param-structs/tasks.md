## 1. Implementation
- [x] 1.1 Inventoried the non-dead_code `#[allow]` sites with file:line (see commit body). Scoped out: raft/discovery `too_many_arguments` (experimental layer per ADR 004), `should_implement_trait` in monitoring/info.rs (justified — inherent + FromStr), `module_inception` in cluster/tests.rs (test)
- [ ] 1.2 Replace each live-code `too_many_arguments` (10: geospatial 3, persistence apply/layer×3/snapshot, replication/replica, websocket) with a parameter struct; remove the `#[allow]` — DELICATE (hot-path signatures + all call sites), best done fresh
- [x] 1.3 Replaced all 4 `type_complexity` with `type` aliases (hash `HScanPage`, transaction `ExecOutcome`, key_manager `FullManager`, main `RecoveredStores`); allows removed, clippy clean
- [ ] 1.4 Fix `result_unit_err` (2, auth/middleware) with a real error type; `while_let_loop` (6, WAL) + `too_many_lines` (2, resp3 advanced) with the idiomatic rewrite; remove those `#[allow]`

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [ ] 2.1 Update or create documentation covering the implementation (note any internal signature changes in the CHANGELOG "Changed" if user-visible)
- [ ] 2.2 Write tests covering the new behavior (existing suite is the regression guard; add a test only where a `result_unit_err` fix introduces a new error path)
- [ ] 2.3 Run tests and confirm they pass (after each suppression-family: `cargo check` + `clippy -D warnings` + full suite, green)
