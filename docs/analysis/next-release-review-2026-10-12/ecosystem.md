# Ecosystem review for the next release

> **Historical analysis.** Describes Synap 1.3.2: reviewed 2026-10-12 against
> `5b1efd1` plus the uncommitted 1.3.2 update, committed in `bfecd0c`
> (2026-10-13). The 1.4.0 release was built from this review (see
> `CHANGELOG.md`, section `[1.4.0]`); its file and line references point at the
> 1.3.2 tree. The code has moved since: paths, line numbers and findings
> describe that revision, not the current tree. Current behavior:
> [1.4.0 guarantees](../../operations/guarantees.md). Current design:
> [persistence](../../specs/PERSISTENCE.md),
> [transports](../../protocol/transports.md),
> [KV store internals](../../internals/kv-store.md) and the
> [documentation index](../../INDEX.md).

Review date: 2026-10-12. Revalidated against the current worktree, including the authorized 1.3.2 dependency/version changes and MCP ServerConfig changes. Findings below remain present. This is static source review, not a claim of executed integration tests. No application code or task state was changed.

Existing work is excluded from proposed new scope: `phase10_typescript-7-swap`, `phase11_v1-3-2-dependabot-fixes`, and `phase12_replace-serde-yaml`. Coordinate CI additions with those tasks; do not recreate dependency upgrades or YAML replacement tasks.

## E-01 - P1 - Migrator and server snapshot formats are incompatible

Evidence: `crates/synap-migrate/src/snapshot.rs:80` discovers only `snapshot-v2-*`. The server writes v3 (`crates/synap-server/src/persistence/snapshot.rs:13`, `:51`). The migrator reads little-endian integers at `crates/synap-migrate/src/snapshot.rs:125`; the server writes big-endian integers at `crates/synap-server/src/persistence/snapshot.rs:67`. Queue/stream records also differ, and the migrator ends its output without the checksum required by the server (`crates/synap-migrate/src/snapshot.rs:398`; `crates/synap-server/src/persistence/snapshot.rs:403`).

Scenario and impact: migrating a normal current server data directory cannot find its v3 snapshot. Selecting an older v2 snapshot does not solve the incompatible codec. This blocks reliable standalone-to-Hub migration; successful migrator self-round-trips would not prove server compatibility.

Correction: reuse the authoritative persistence codec through an appropriate shared module, support supported snapshot versions, and define how pending WAL records are replayed and scoped before producing the migrated state. Preserve collection, queue, and stream metadata. Do not fix only the filename filter.

Acceptance: generate fixtures using the real server, including supported v2/v3 formats, all persisted structures, queue/stream metadata, and writes after the snapshot. Migrate and boot the real server in Hub mode; verify scoped data and supported expiry semantics against a pre-migration baseline. Verify rollback and clear rejection of corrupt/unsupported input before modifying the original data.

## E-02 - P1 - Configuration editor claims success without a server write

Evidence: `gui/src/views/ConfigView.vue:278` updates only local `originalConfig`; `:280` displays `Configuration saved successfully!`. There is no apply request in `saveConfig`. Load failures substitute a sample configuration at `:195`.

Scenario and impact: an operator changes configuration and clicks Save Changes. The dirty indicator clears and success appears, although server configuration has not changed. The interface can mislead operators about durability or security settings.

Correction: implement a supported configuration read/apply contract with explicit acknowledgement and restart requirements, or disable editing until supported. Display failed reads as errors; do not display sample data as the connected server's configuration.

Acceptance: change a supported setting, verify server readback, and reload the view. Failed or denied updates retain dirty state and show actionable errors. Unsupported settings and restart-required changes have explicit outcomes. Failed loading cannot lead to saving sample configuration as actual state.

## E-03 - P2 - Structure inspector calls nonexistent command names

Evidence: `gui/src/services/api.ts:740` sends `kv.type`, and `:754` sends `kv.memory`. Server dispatch registers `key.type` and `memory.usage` at `crates/synap-server/src/server/handlers/mod.rs:342` and `:352`. `gui/src/views/StructuresView.vue:308` checks the failed type response and clears the inspector.

Scenario and impact: inspect an existing hash, list, set, or sorted set. Type lookup fails before the structure fetch, leaving the inspector empty. Memory fallback requests also use an unsupported command.

Correction: align command names and response contracts, or use the existing REST routes `/key/{key}/type` and `/memory/{key}/usage`. Prefer contract tests tied to the actual server over duplicate mocked command catalogs.

Acceptance: create each supported structure through the server and inspect its type and contents in the GUI. Verify memory response parsing, missing keys, permission errors, and encoded key names.

## E-04 - P2 - Native transports discard INCR and DECR amounts

Evidence: CLI sends optional `amount` at `crates/synap-cli/src/main.rs:170` and `:182`; HTTP consumes it at `crates/synap-server/src/server/handlers/kv_cmd.rs:173`. Native command mapping at `sdks/rust/src/transport/mapping.rs:56` and `:57` always emits one-argument `INCR`/`DECR`.

Scenario and impact: `INCR count 10` changes the counter by 10 on HTTP but by 1 on RPC/RESP3. Changing transport silently changes application arithmetic.

Correction: map explicit amounts to INCRBY/DECRBY with consistent validation and overflow handling. Retain default increment/decrement behavior when no amount is supplied. Reject malformed CLI amounts instead of silently treating them as 1.

Acceptance: run identical CLI/SDK scenarios against HTTP, RPC, and RESP3 for omitted, positive, zero, and negative amounts. Assert returned and stored values; verify malformed input and integer overflow fail consistently without an unintended mutation.

## E-05 - P2 - CLI PING exits successfully when the server is unreachable

Evidence: `crates/synap-cli/src/main.rs:287` converts every connectivity error into `Ok("Server unreachable")`. The noninteractive success branch at `:403` consequently returns success.

Scenario and impact: a health script checks `synap-cli ... PING` by process status while the server is stopped. It receives exit code zero and can report a false healthy result. Authentication failures are also collapsed into the same successful result.

Correction: propagate failures to the nonzero exit path and preserve useful diagnostics on stderr. Prefer a proper protocol ping contract to a DBSIZE connectivity surrogate where available.

Acceptance: healthy endpoint exits zero; connection refusal, timeout, and denied credentials exit nonzero. Assert exit status and output streams from the executable, not only the command helper.

## E-06 - P2 - Pull-request validation omits GUI and non-Rust contract gates

Evidence: `.github/workflows/rust-ci.yml:43` checks the Rust workspace. Non-Rust SDK tests appear in the release/manual publishing workflow (`.github/workflows/sdk-publish.yml:23`), not a pre-merge SDK matrix. Current workflows do not run GUI validation or the cross-language interoperability harness on pull requests. Coverage at `.github/workflows/rust-ci.yml:127` generates library LCOV without an explicit failure threshold.

Scenario and impact: a changed server command contract or broken GUI request can merge despite green Rust checks, then fail in consumers or during publication. E-03 and E-04 illustrate the contract classes these gates should catch.

Correction: add pre-merge SDK validation and server-backed contract cases, Vue-aware GUI checking and primary-flow smoke tests, plus artifact-consumption checks before publishing. Measure current coverage by component and test mode before choosing enforceable gates. The repository states a >=95% policy; explicitly reconcile measured coverage and that policy rather than silently bypassing it or claiming compliance without measurements.

Acceptance: intentionally incorrect command names and changed amount mapping fail the relevant pull-request checks. Test the package artifacts against a built server. Publish measured coverage and enforce documented thresholds; a drop below the configured gate must fail. Keep dependency/version work under the existing tasks named above.

## Separate improvement proposals

- Controlled performance baseline: `.github/workflows/rust-ci.yml:71` only compiles benchmarks with `--no-run`. Add scheduled runs on controlled hardware with latency, throughput, memory, workload, build, and runner metadata. Establish repeatability before choosing regression thresholds; avoid noisy shared-runner latency gates.
- Historical documentation hygiene: `docs/analysis/redis-parity-deep-dive.md:51` references the removed `synap-protocol` crate, and `docs/analysis/synap-vs-redis/set-deep-dive.md:7` references the former monolithic handler path. Label historical reviews with their source revision and link current architecture. Revalidate old conclusions before creating tasks; do not present moved paths as proof that an old bug remains.
- Reproducible binary releases: `.gitignore:5` ignores `Cargo.lock`, and `git ls-files Cargo.lock` returns no tracked lockfile. Release jobs select moving `nightly` at `.github/workflows/release.yml:46`; Rust CI likewise selects moving nightly at `.github/workflows/rust-ci.yml:34`. Define an approved lockfile/toolchain policy for this application workspace, capture dependency resolution per release, and verify fresh checkouts resolve the same graph. Prefer a committed application lockfile and locked builds unless a documented alternative provides equivalent reproducibility. This is separate from applying the dependency bumps already covered by phase11.
