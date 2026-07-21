## 1. Implementation
- [x] 1.1 Inventoried the non-dead_code `#[allow]` sites with file:line (see commit body). Scoped out: raft/discovery `too_many_arguments` (experimental layer per ADR 004), `should_implement_trait` in monitoring/info.rs (justified — inherent + FromStr), `module_inception` in cluster/tests.rs (test)
- [x] 1.2 Replaced all 10 live-code `too_many_arguments` with parameter structs: `GeoQueryOptions`/`GeoSearchParams` (geospatial ×3), `StoreRefs`/`StoreArcs` (persistence apply/layer×3/snapshot + `ReplicaNode::new`), `ZAddOptions` reuse (`log_zadd`), `StreamSocketParams` (websocket); allows removed, clippy `-D warnings` + 90 suites green
- [x] 1.3 Replaced all 4 `type_complexity` with `type` aliases (hash `HScanPage`, transaction `ExecOutcome`, key_manager `FullManager`, main `RecoveredStores`); allows removed, clippy clean
- [x] 1.4 Fixed `result_unit_err` (2): auth middleware returns `AuthRejection` enum (UnknownApiKey/InvalidApiKey/MalformedHeader/InvalidCredentials, thiserror, always 401); `while_let_loop` (6, WAL): `while let Ok(size) = read_u64()`; `too_many_lines` (2, resp3): extracted `parse_geo_option_tail` + `parse_geosearch_args` helpers; all `#[allow]` removed

## 2. Tail (docs + tests — check or waive with tailWaiver)
- [x] 2.1 CHANGELOG "Changed" documents both refactors (parameter structs + lint fixes); internal-only, no public API/wire change
- [x] 2.2 Added `test_auth_rejection_reports_specific_variant` covering the new `AuthRejection` error paths (UnknownApiKey, MalformedHeader, InvalidCredentials); existing suite guards the rest
- [x] 2.3 After each family: `cargo check` + `clippy -D warnings` clean; full suite green (90 test binaries, 0 failures) after 1.2 and after 1.4
