# phase6 stability: unwrap-triage patterns + the "442 unwraps" was test code
**Source**: manual
**Date**: 2026-07-09
**Related Task**: phase6_v1-stability-hardening
**Tags**: analysis:synap-v1-release, phase6, unwrap-triage, flaky-tests, config, replication
phase6 stability hardening. Key correction: the proposal's "442 core unwraps / 156 flaky markers" counted TEST code. Real non-test counts: ~33 in synap-core, ~93 in synap-server. Measure accurately with `awk '/#\[cfg\(test\)\]/{exit}{print}' file | grep -cE '\.unwrap\(\)|\.expect\(|panic!\('` per file, and exclude whole-file test modules (e.g. store_tests.rs, bitmap/tests.rs — included via `#[cfg(test)] mod store_tests;` so no inner cfg(test) marker).

Unwrap-triage patterns applied (all gate-green):
- `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()` (25 core + 10 server) → `.unwrap_or_default()` (0 on a pre-1970 clock). Uniform perl: `s/(\.duration_since\([^)]*UNIX_EPOCH\)\s*\n\s*)\.unwrap\(\)/${1}.unwrap_or_default()/g`.
- Fixed-size shard arrays: `Vec::try_into().unwrap()` → `std::array::from_fn(|_| ...)` (infers N from the field type; no fallible conversion).
- Prometheus `register_*_vec!(...).unwrap()` (43 in metrics/mod.rs lazy_static) → `.expect("metric registration uses a static, unique name")` (uniform: `s/\)\.unwrap\(\);/).expect(...);/g`).
- `json!({...}).as_object().unwrap()` (24 in mcp_tools) → expect (literal is always an object).
- main.rs startup config panics → `error!` + `std::process::exit(1)` (needs `use tracing::error`).
- Remaining invariants documented via `.expect("...")`: just-inserted map get, len-checked match arm, plain-struct JSON serialization, numeric→HeaderValue parse, hardcoded socket addr, just-peeked heap pop.

Markers (1.8): 13 TODO/FIXME (7 server + 6 core, all in disabled-by-default hub/cluster features) → replaced with `hivellm/synap#230-#233` refs (the pre-commit hook blocks the literal word TODO/FIXME in source).

Deflake finding (1.2/1.3): `test_concurrent_writes_during_sync` (replication_integration.rs) was `#[ignore = "flaky timing"]`. Replacing the fixed sleep(2s) with poll-until-500-keys and running 10x proved it fails DETERMINISTICALLY, not flakily: a replica joining mid-write-stream reliably misses in-flight writes (real gap in partial-for-v1.0 replication). Kept ignored with accurate justification → #234; poll left in place so it is re-enable-ready. Lesson: verify a "flaky" label by running N times before re-enabling — it may be genuinely broken.

Config (1.7): RESP3+SynapRPC default to loopback in code (e71ed87); also aligned config/config.yml (was host 0.0.0.0 despite a "bind loopback" comment) → 127.0.0.1 with an explicit-exposure note. Main HTTP API intentionally stays 0.0.0.0 (out of the finding's scope).