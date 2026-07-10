# Findings — synap-v1-release

Numbered findings with evidence. Confidence: High = verified at file:line; Medium = needs
per-item triage during implementation.

## Workstream A — Workspace restructure

### F-001 — Current layout is flat; target is `crates/` following Nexus/Vectorizer
- Evidence: root `Cargo.toml:3` → `members = ["synap-server", "synap-cli", "synap-migrate", "sdks/rust"]`.
  Siblings: `../Nexus/Cargo.toml` (`crates/nexus-{core,protocol,server,cli,bench}`),
  `../Vectorizer/Cargo.toml` (`members = ["crates/*", "sdks/rust"]`).
- Target: `crates/synap-{core,protocol,server,cli}` + `crates/synap-migrate`, keep `sdks/rust` as member.
- Impact: HIGH (release-defining, mechanical). Confidence: High.

### F-002 — `synap-server` is a ~60K-LOC, 23-module monolith to decompose
- Evidence: `synap-server/src/lib.rs` declares 15 top modules. LOC: `core/` 16,526 (23 files),
  `server/` 15,939 (30 files), `protocol/` 8,289 (19 files), `persistence/` 4,049, `auth/` 3,392,
  `hub/` 3,091, `cluster/` 2,781, `replication/` 2,238, plus `monitoring/ cache/ simd/ scripting/ metrics/ compression/`.
- Proposed mapping:
  - `synap-core` ← `core/`, `cache/`, `compression/`, `simd/` (leaf crate, no server/protocol deps)
  - `synap-protocol` ← pure wire: `protocol/envelope.rs`, `protocol/resp3/{parser,writer}.rs` + RESP value type,
    `protocol/synap_rpc/{codec,types}.rs`
  - `synap-server` ← `server/`, `auth/`, `cluster/`, `hub/`, `replication/`, `persistence/`, `metrics/`,
    `monitoring/`, `scripting/`, **plus** dispatch layers `protocol/resp3/command/`, `protocol/synap_rpc/dispatch/`
- Impact: HIGH. Confidence: High.

### F-003 — BLOCKER: protocol dispatch is coupled to `AppState` + `core` + `scripting`
- Evidence: `protocol/resp3/command/advanced.rs:1-3` imports `AppState`,
  `crate::core::geospatial::DistanceUnit`, `crate::scripting::ScriptExecContext`;
  `protocol/synap_rpc/dispatch/mod.rs:8` → `use crate::server::handlers::AppState`;
  `dispatch/collections.rs:2` → `crate::core::sorted_set::ZAddOptions`.
  `resp3/command/mod.rs` is 1,157 LOC of `AppState`-bound handlers.
- Consequence: a naive "move `protocol/` → `synap-protocol`" creates a `protocol → server` cycle.
  Only parser/writer/codec/wire-types go to `synap-protocol`; `command/` and `dispatch/` are
  relocated inside `synap-server`.
- Impact: HIGH. Confidence: High.

### F-004 — `AppState` is the server seam aggregating all core stores
- Evidence: `server/handlers/mod.rs:65-90` — 20+ `Arc<...>` fields spanning core stores
  (KVStore, HashStore, ListStore, SetStore, SortedSetStore, HyperLogLogStore, BitmapStore,
  GeospatialStore, StreamManager, PartitionManager, ConsumerGroupManager, PubSubRouter, QueueManager),
  `persistence`, `monitoring`, `cluster`, `hub`, `scripting`, `transaction`.
- Consequence: `AppState` lives in `synap-server` and is the natural home for dispatch.
- Impact: MEDIUM. Confidence: High.

### F-005 — `sdks/rust` should consume `synap-protocol` (removes wire-type duplication)
- Evidence: `sdks/rust/Cargo.toml` pulls `rmp-serde`/`reqwest` and re-derives wire types across
  21 modules; `protocol/synap_rpc/types.rs:11,88,99` defines `SynapValue`/`Request`/`Response` —
  the exact types the SDK re-implements. Same lever Vectorizer's split cites ("SDK type duplication").
- Impact: MEDIUM (wire-drift prevention). Confidence: Medium (needs per-type diff).

### F-006 — Version + package-field drift across crates
- Evidence: `[workspace.package] version = "0.12.0"` (root `Cargo.toml:10`) but **0.13.0 is released**
  (`CHANGELOG.md:10`, branch `release/v0.13.0`, commit `3f00610`). `synap-server/Cargo.toml:3`
  hardcodes `0.12.0`; `sdks/rust/Cargo.toml:3-4` hardcodes version **and** edition; only
  `synap-cli`/`synap-migrate` inherit via `.workspace = true`.
- Impact: HIGH — reconcile to a single inherited `1.0.0-rc` baseline. Confidence: High.

## Workstream B — Open dependabot PRs

### F-007 — 8 open PRs: 3 Cargo (individual review), 5 npm (batch-mergeable)
- Evidence: `gh pr list` → #222 sysinfo 0.33→0.39, #223 mlua 0.11.5→0.12.0, #224 rmcp 1.5.0→2.1.0 (Cargo);
  #225 vitest 4.1.9→4.1.10, #226 @vitest/coverage-v8 4.1.10, #227/#228/#229 typescript-eslint 8.62.1→8.63.0 (TS SDK dev deps).
- Impact: MEDIUM. Confidence: High.

### F-008 — The Cargo majors are breaking and touch load-bearing subsystems
- Evidence: `rmcp` used in 5 files (MCP layer); every prior rmcp bump broke the API
  (`CHANGELOG.md:159,405`) — 1.5→2.1 is a two-major jump, highest risk.
  `mlua 0.11→0.12` hits `scripting/mod.rs` (Lua engine, features `lua54,async,send,serialize,vendored`).
  `sysinfo 0.33→0.39` hits `server/metrics_handler.rs` — process-metrics code just rewritten in 0.13.0.
- Consequence: do NOT batch-merge; each needs its own compile+test gate,
  order sysinfo → mlua → rmcp.
- Impact: MEDIUM-HIGH. Confidence: High.

### F-009 — npm/TS-SDK PRs are dev-deps, safe to batch
- Evidence: repo practice is batch-bumping TS deps (`75d4ff7`, `1e6953e`). #225-229 are vitest/eslint tooling.
- Impact: LOW. Confidence: High.

### F-010 — Stale dependabot branches on origin need pruning
- Evidence: superseded duplicates on origin: `eslint-10.3.0` vs `10.6.0`, `msgpackr-2.0.1` vs `2.0.4`,
  `vitest-4.0.4`/`4.1.6`, `typescript-eslint-8.59.3` vs `8.62.1` (+ plugin/parser variants), `types/node-26.0.1`.
- Impact: LOW (hygiene). Confidence: High.

## Workstream C — Stability

### F-011 — Timing-dependent tests are a flakiness reservoir
- Evidence: 156 `sleep(`/`#[ignore]`/`flaky` occurrences across `synap-server` (1,441 test fns).
  CHANGELOG documents the recurring pattern: "deflake stream replication test" (`483be53`),
  WS `parseValue` hang on multi-chunk responses (`CHANGELOG.md:151`), s2s-tests hang prevention (`CHANGELOG.md:429-431`).
- Recommendation: flaky-test audit; replace `sleep`-based sync with barriers/`tokio::sync::Notify`.
- Impact: HIGH for the 1.0 gate. Confidence: Medium (count is a proxy; needs a real flake run).

### F-012 — `unwrap()`/`expect()`/`panic!` in hot paths violates the project Rust rule
- Evidence: `.claude/rules/rust.md` §4 forbids them in non-test code; 65 non-test files contain them,
  `core/` alone has 442 `unwrap()` hits (a portion are in-file `#[cfg(test)]`, but not all).
  `core/` is the request hot path.
- Impact: HIGH — a reachable `unwrap` on user input is a remote DoS for a 1.0 datastore.
  Confidence: Medium (needs per-hit triage prod vs test).

### F-013 — `Resp3Config` default is contradictory
- Evidence: `config.rs:218` `#[serde(default)] pub enabled: bool` → **false** when the YAML omits it,
  but `config.rs:236-243 impl Default` sets `enabled: true`. Struct-built config enables RESP3;
  YAML-loaded config missing the field disables it.
- Impact: MEDIUM — environment-dependent behavior of the flagship Redis-compat listener. Confidence: High.

### F-014 — Inconsistent listener bind defaults
- Evidence: `config.rs:232` RESP3 defaults `127.0.0.1` (loopback) but SynapRPC
  (`default_synap_rpc_host`) defaults `0.0.0.0` (all interfaces).
- Impact: MEDIUM (security posture). Confidence: High.

### F-015 — 13 TODO/FIXME markers remain in `synap-server/src`
- Evidence: `grep TODO|FIXME|XXX|HACK` → 13 hits.
- Impact: LOW-MEDIUM — resolve or convert to tracked issues before 1.0. Confidence: High.

## Workstream D — Performance & Redis comparison

### F-016 — Prior `synap-vs-redis` analysis is partly OUTDATED; two CRITICAL blockers resolved
- Evidence: `docs/analysis/synap-vs-redis/findings.md` lists "No RESP" and "No pipelining" as CRITICAL,
  but RESP3 is implemented and wired: `main.rs:524-530` (`spawn_resp3_listener`, port 6379),
  `protocol/resp3/{parser,writer,server}.rs`, 94-command dispatch (`resp3/command/mod.rs`, 1,157 LOC).
  Old F-004 (write lock on GET) and SET-allocation issues are marked RESOLVED (AtomicU32 GET,
  −13% to −18% SET latency); eviction shipped with 6 Redis policies.
- Consequence: v1.0 Redis story is **benchmark + tune**, not build. The comparison-table architecture
  point stands: 64-shard `parking_lot::RwLock` (Synap) vs single-threaded event loop (Redis).
- Impact: HIGH (prevents redoing shipped work). Confidence: High.

### F-017 — Synap ships TWO binary protocols (RESP3 + SynapRPC/MessagePack)
- Evidence: `protocol/mod.rs` exports both; `main.rs:534-539` spawns SynapRPC listener (port 15501).
  RESP3 = Redis-client compat; SynapRPC = native MessagePack RPC for first-party SDKs.
- Consequence: benchmark `redis-benchmark` → Synap RESP3 vs Redis 7, plus a separate SynapRPC measurement.
- Impact: MEDIUM. Confidence: High.

### F-018 — Remaining Redis-parity/perf gaps to close or explicitly defer for v1.0
- Still open from `synap-vs-redis`: sequential MGET/MSET (parallelize by shard), blocking ops
  (BLPOP/BRPOP/BZPOPMIN), PSUBSCRIBE/keyspace notifications, HSCAN/SSCAN/ZSCAN cursors,
  allocator (mimalloc/jemalloc) + IO threads, LFU eviction. Pipelining depth in `resp3/server.rs`
  to be verified (read-N-before-flush).
- Impact: MEDIUM — decide per item: ship in 1.0 or document as post-1.0. Confidence: Medium.

### F-019 — Structural debt cited by the old analysis is already fixed (verify, don't redo)
- Evidence: old "handlers.rs is 11,595 lines" — now split into `server/handlers/` (18 files).
  Remaining large core files (`list.rs` 37K, `queue.rs` 37K, `geospatial.rs` 35K, `sorted_set.rs` 30K)
  are natural `synap-core` residents, not release blockers.
- Impact: LOW. Confidence: High.
