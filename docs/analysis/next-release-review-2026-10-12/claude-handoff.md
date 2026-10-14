# Claude handoff: next-release tasks

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

## Requested outcome

The user requested a general project review, improvement recommendations, and delivery to Claude to open tasks for the next release. Read this directory's README.md, security.md, durability.md and ecosystem.md. Create actionable Rulebook tasks from the evidence. This is planning, not authorization to implement fixes or ship a release.

Review date: 2026-10-12. Initial commit: `5b1efd1`. Findings were subsequently rechecked against the uncommitted 1.3.2 dependency update. Another interactive Claude session was working on that update; the user explicitly requested waiting for it to finish before proceeding with competing work. Do not interfere with that session or overwrite its changes.

## Required process

1. Read AGENTS.md, AGENTS.override.md, .rulebook/specs/RULEBOOK.md and current task state. Inspect existing task IDs before creating any. Recheck source references if the tree has moved since review.
2. Use Rulebook MCP for creation, updates and validation. Pass projectId when workspace mode requires it. Use available knowledge/learning/decision lookup tools and follow the local specification format. If a required integration is unavailable, report the concrete blocker instead of claiming success or silently creating task directories by hand.
3. Preserve phase10_typescript-7-swap, phase11_v1-3-2-dependabot-fixes, phase12_replace-serde-yaml and any newly created external task. Do not duplicate their scope, mark their items complete, change versions, or archive tasks.
4. Allocate unused sequential phase numbers. The suggested sequence below assumes phase13 is still free; rename consistently if another session consumed a number. Each proposal must link this review and list finding IDs, priority, impact, scope, dependencies and compatibility risks.
5. Each task needs proposal.md with Why and What Changes, tasks.md containing only unchecked checklist lines, and at least one specs/<module>/spec.md. Requirements must use SHALL/MUST, with four-hash Scenario headings and Given/When/Then cases. Include documentation, meaningful regression tests, and verification in every checklist.
6. Validate every created task using Rulebook. Leave implementation items unchecked and status pending. Do not start fixes, commit, merge, publish, push, delete, or amend anything. Do not change repository settings or bypass approval controls.
7. Write docs/analysis/next-release-review-2026-10-12/claude-receipt.md with actual task IDs, finding-to-task mapping, dependencies, validation outcomes, any changed priorities, and unresolved blockers. Include a separate explicit statement of whether all 21 finding IDs have coverage. Do not claim acceptance tests were run merely because tasks describe them.

## Suggested task sequence

| Proposed ID | Evidence | Scope and acceptance emphasis | Dependencies |
| --- | --- | --- | --- |
| phase13_api-key-delegation-boundaries | S-01 | Authenticated issuance and bounded permission delegation; reject broader wildcard/action grants and anonymous issuance in authenticated mode | None |
| phase14_shared-command-authorization | S-02, S-05 | Explicit principal/resource context for envelope HTTP, RESP3 and Thunder; cover global, multi-key, script and transaction operations with denial tests | phase13 policy alignment |
| phase15_mcp-umicp-request-identity | S-04 | Remove thread-local identity dependence, bind sessions and dispatch context, reject missing context in authenticated mode; interleaved-session and UMICP tests | phase14 authorization contract |
| phase16_hub-tenant-enforcement | S-03 | Wire production Hub authentication, fail closed on missing tenant, define local/Hub mode policy, test isolation on every enabled transport | phase14, phase15 |
| phase17_request-resource-limits | S-06, S-07 | Enforce configured rate limits and bounded UMICP body consumption; test refill, 429, 413, chunked input and authentication ordering | phase16 identity policy |
| phase18_wal-lifecycle-and-repair | D-01, D-02, D-06 | Independent flush timer, Never-mode buffer semantics, shutdown barrier, group error propagation, safe append after torn tail; fault injection and two-restart tests | None; precedes format/commit changes |
| phase19_atomic-transaction-records | D-03 | Versioned transaction framing, recovery all-or-nothing and explicit replication grouping; truncate at every byte boundary and preserve backward compatibility | phase18 |
| phase20_consistent-snapshots-and-expiry | D-04, D-07 | Coherent state/WAL cutoff and absolute expiry across snapshot/WAL/replication; controlled concurrency and clock tests | phase18, phase19 |
| phase21_queue-replay-identity | D-05 | Restore full message identity/metadata and replay ACKs correctly across ready/pending state; WAL/snapshot/replica parity | phase18, phase20 |
| phase22_string-mutation-durability | D-08 | Correct APPEND/SETRANGE durable representation with exact bytes, expiry and mutation ordering; restart/replica/concurrency tests | phase18, phase20 |
| phase23_migrator-server-format-parity | E-01 | Shared authoritative codec, supported v2/v3 fixtures, WAL-tail treatment, all structures, safe rollback and real-server restore | phase16, phase19, phase20, phase21 |
| phase24_native-counter-and-cli-status | E-04, E-05 | HTTP/RPC/RESP3 amount equivalence and executable PING exit/error contract | None |
| phase25_gui-server-contracts | E-02, E-03 | Truthful configuration load/save and correct structure inspection; server readback, permissions, restart requirements and smoke tests | phase14 authorization behavior |
| phase26_pr-contract-and-release-gates | E-06; ecosystem reproducibility proposal | SDK/GUI/server contracts before merge, installed-artifact tests, measured coverage policy, application lockfile/toolchain decision and reproducible builds | Prior fixes supply regression cases; coordinate with existing release work |
| phase27_operational-docs-and-performance-baseline | Separate improvement proposals | Current architecture/guarantees documentation, historical-review revision labels, repeatable controlled-runner latency/throughput/memory baselines | phase18 through phase26 for final behavior |

These groups are a proposal. Split an oversized group when needed, but preserve complete finding coverage and a clear sequential execution order. Do not silently downgrade or omit a finding; record evidence for any change in the receipt.

## Release and sequencing decisions

- Do not assign or bump a next version during task creation. Most fixes are patch candidates; persisted-format and authorization compatibility need an explicit release decision.
- Existing task ordering is mandatory. The TypeScript 7 task is externally blocked; document how that affects the proposed sequence and identify any required sequencing decision. Do not pretend newly numbered security tasks automatically override the repository's earlier-phase rule.
- Retain Thunder as the external binary wire dependency. Do not recreate synap-protocol.
- Honor ADR 004: experimental cluster consensus code is intentionally retained. A new production consensus project is outside this hardening review's confirmed findings.
- Recovery changes must specify how old WAL/snapshot data is read, how unsupported versions fail, and how operators roll back safely. Do not change only documentation to hide a violated durability promise.
- No benchmark speedup or coverage percentage is established by this static review. Measure baselines; document and enforce chosen gates without fabricated claims or skipped tests.

## Validation already performed by the reviewing session

- Initial 1.3.1 tree: cargo fmt --all --check, cargo check --workspace --all-targets --locked --offline, and cargo clippy --workspace --all-targets --locked --offline -- -D warnings passed. Cargo reported a future-incompatibility notice for its then-resolved redis v1.5.0 dev dependency.
- Updated 1.3.2 tree: cargo check --workspace --all-targets --locked --offline and cargo clippy --workspace --all-targets --locked --offline -- -D warnings passed.
- No runtime exploit, failure-injection regression, full test suite, coverage run or benchmark was executed by the reviewing session. Other Claude session results must be attributed separately if incorporated.
