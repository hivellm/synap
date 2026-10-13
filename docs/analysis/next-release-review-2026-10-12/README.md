# Synap next-release review

Review date: 2026-10-12 (America/Sao_Paulo).

## Findings first

The next release should prioritize authorization consistency and recovery correctness. Current transports do not enforce the same access rules, and several successful write paths cannot reliably reconstruct the acknowledged state after restart. Dependency upgrades alone do not address these defects.

| Area | Findings | Main consequence | Priority |
| --- | --- | --- | --- |
| Authorization and tenant isolation | S-01 through S-05 | Restricted identities can reach forbidden operations; Hub requests lack required tenant context | P1 |
| WAL, snapshots, replay and replication | D-01 through D-08 | Lost writes, partial transactions, duplicate queue delivery, incorrect expiry or restored values | P1 |
| Migration utility | E-01 | Current server snapshots cannot be migrated reliably | P1 |
| GUI configuration | E-02 | Interface claims settings were saved without applying them | P1 |
| Resource limits | S-06, S-07 | Configured rate limits are inactive; UMICP body buffering is unbounded | P2 |
| Client contracts and CLI health checks | E-03 through E-05 | Broken inspection, wrong counter amounts, false successful health checks | P2 |
| Verification and releases | E-06 | Client/server incompatibilities can escape PR checks | P2 |

P1 means fix before the next production release that promises the affected behavior. P2 means schedule in the next hardening cycle. These are contextual engineering priorities, not CVSS scores. Exposure depends on enabled transports and deployment configuration.

## Detailed evidence

- [Security and transport authorization](security.md): S-01 through S-07.
- [Durability, recovery and replication](durability.md): D-01 through D-08.
- [SDKs, CLI, GUI, migration and CI](ecosystem.md): E-01 through E-06, plus separate improvement proposals.
- [Claude handoff](claude-handoff.md): task creation scope, order and acceptance requirements.

Each finding names current code locations, a concrete failure scenario, a recommended direction, and regression tests. The scenarios are derived from source inspection unless explicitly listed as executed under validation. No production service was probed or modified.

## Baseline and concurrent work

The initial baseline was commit `5b1efd1`, workspace version `1.3.1`. Another Claude session then changed the working tree for `phase11_v1-3-2-dependabot-fixes`. Review paused at the user's request until that session became idle. Findings were rechecked against the resulting uncommitted `1.3.2` working tree; dependency updates and the MCP `ServerInfo` to `ServerConfig` change do not remove them.

Existing work is preserved, including `.claude/settings.json`, dependency manifests and lockfiles, release documentation, and Rulebook records. This review adds analysis and planning artifacts only. It does not finish, merge, publish, or certify the separate 1.3.2 release task.

The next version number is intentionally not assigned. Most fixes can be backported to a patch release. Changes to persisted formats, authorization semantics, or enabled transports require an explicit compatibility and migration decision before choosing a version.

## Validation

Before the concurrent dependency update:

- `cargo fmt --all --check`: passed.
- `cargo check --workspace --all-targets --locked --offline`: passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed, with Cargo's separate future-incompatibility notice for the then-resolved `redis v1.5.0` dev dependency.

After the update:

- `cargo check --workspace --all-targets --locked --offline`: passed on the 1.3.2 working tree.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed on the 1.3.2 working tree.
- No full test suite, runtime exploit, fault-injection regression, coverage run or benchmark was executed by this reviewing session. Acceptance tests in the detailed reports remain proposed work.

These checks establish build and lint status, not runtime safety. No coverage percentage, benchmark improvement, crash-injection result, or cross-language interoperability result is claimed without execution. `Cargo.lock` is not tracked, so a commit hash alone does not fully identify the dependency resolution used by these checks.

## Architecture and improvement direction

1. Introduce an explicit operation context containing authenticated principal, effective permissions, tenant and limits. Reuse authorization and resource resolution across REST, command envelopes, MCP, UMICP, RESP3 and Thunder. Test each entry point; authentication at connection or router level alone is insufficient.
2. Define one durable mutation boundary. Mutation result, WAL representation, replication record and client acknowledgement must agree. Typed operations and a shared application path should replace transport-specific logging, which currently diverges from in-memory mutations.
3. Specify recovery invariants: an acknowledged durable write survives recovery; a transaction is all-or-nothing; snapshots have a matching committed WAL position; message identity and absolute expiry survive restart. Drive design with fault injection and restart tests.
4. Share persistence codecs between server and migration utility. Add versioned fixtures produced by real server releases and exercise forward/backward compatibility instead of relying on each component's private round-trip tests.
5. Move SDK and GUI contracts into PR verification. Exercise installed/built client artifacts against a real isolated server, including negative authorization cases and transport equivalence. Keep raw throughput benchmarks on a controlled runner, with p50/p95/p99 latency, memory, workload, durability mode and hardware recorded.
6. Refresh operational documentation from verified behavior. Distinguish local engine microbenchmarks from network benchmarks, memory safety from leak freedom, and tested cluster primitives from production consensus/failover.

Retain the existing separation of `synap-core`, server adapters and external `thunder-rpc`. Do not recreate `synap-protocol`. ADR 004 explicitly preserves the experimental cluster consensus modules; their staged status is not grounds to delete or gate them. Any future wiring work needs its own runtime design and failure tests.

## Existing tasks to preserve

- `phase10_typescript-7-swap`: external typescript-eslint compatibility dependency; do not duplicate.
- `phase11_v1-3-2-dependabot-fixes`: dependency/release work in progress; do not claim it complete.
- `phase12_replace-serde-yaml`: existing YAML maintenance work; do not duplicate.

New review tasks must use the next unused phase numbers, reference finding IDs, state dependencies, and remain unchecked until implemented and verified. The existing task-order policy needs an explicit sequencing decision if the externally blocked TypeScript task would otherwise prevent urgent security work.
