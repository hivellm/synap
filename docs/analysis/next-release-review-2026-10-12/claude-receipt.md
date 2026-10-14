# Claude receipt: next-release tasks

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

Date: 2026-10-13. Tree: `main` at `1f0f2dd` (1.3.2 merged locally, not pushed or tagged).

## Decision that changes the handoff

The user asked for every finding to ship in **one version**. The target is
**1.4.0**, recorded in ADR 006 (`ship-the-next-release-review-hardening-as-one-1-4-0-release`).
It is a minor release because 1.4.0 keeps reading 1.3.x WALs and v3 snapshots
and the wire protocols are unchanged. Downgrading after 1.4.0 has written data
is unsupported, and the phase30 migration guide documents it. Every proposal
carries `Target release: 1.4.0`, and phase30 is the release task.

## Verification of the findings

Before creating tasks, every one of the 21 findings was rechecked by reading the
current source on `main`. All 21 still hold. The rechecks found six things the
review did not state:

| ID | What was found | Where it went |
| --- | --- | --- |
| **N-01 (new, P1)** | RESP3 and SynapRPC dispatchers apply writes in memory and reply without calling the persistence layer (`protocol/resp3/command/kv.rs` `cmd_set`, `protocol/synap_rpc/dispatch/kv.rs` `SET`). The only persistence call under `crates/synap-server/src/protocol/` is `log_transaction` in EXEC, so non-transactional native writes are neither in the WAL nor replicated | phase21 |
| D-08 extension | The envelope path logs empty values too (`server/handlers/kv_cmd.rs:422`, `:494`), not only REST | phase21 |
| S-04 aggravation | rmcp 3.5 runs session/tool work in tasks it spawns (`tokio::spawn` in `streamable_http_server/tower.rs`), so the thread-local identity is usually absent in the tool handler and the absent-context branch allows the call. MCP permissions are likely not enforced at all in authenticated mode, not just under unlucky scheduling | phase17 (item 1.1 confirms against rmcp) |
| S-01 extension | `allowed_ips` entries that fail to parse are dropped by `filter_map`, which can leave a key with no IP restriction | phase15 |
| D-06 extension | The scanner trusts the frame length: `vec![0u8; size]` on a corrupt length can request an enormous allocation and abort startup. The checksum is read but never verified | phase20 |
| E-02 extension | The configuration view loads with `admin.config`, a command no server dispatcher defines, so it always falls back to the sample and presents it as the server's configuration | phase27 |

## Tasks created

| Task | Findings | Priority | Depends on |
| --- | --- | --- | --- |
| phase15_api-key-delegation-boundaries | S-01 | P1 | none |
| phase16_shared-command-authorization | S-02, S-05 | P1 | phase15 |
| phase17_mcp-umicp-request-identity | S-04 | P1 | phase16 |
| phase18_hub-tenant-enforcement | S-03 | P1 | phase16, phase17 |
| phase19_request-resource-limits | S-06, S-07 | P2 | phase18 |
| phase20_wal-lifecycle-and-repair | D-01, D-02, D-06 | P1 | none |
| phase21_durable-writes-on-every-transport | N-01, D-08 | P1 | phase20 |
| phase22_atomic-transaction-records | D-03 | P1 | phase20 |
| phase23_consistent-snapshots-and-expiry | D-04, D-07, D-08 (same-key ordering) | P1 | phase20, phase21, phase22 |
| phase24_queue-replay-identity | D-05 | P1 | phase20, phase23 |
| phase25_migrator-server-format-parity | E-01 | P1 | phase18, phase22, phase23, phase24 |
| phase26_native-counter-and-cli-status | E-04, E-05 | P2 | none |
| phase27_gui-server-contracts | E-02, E-03 | P1 / P2 | phase16 |
| phase28_pr-contract-and-release-gates | E-06, reproducible releases | P2 | phase26, phase27 |
| phase29_operational-docs-and-performance-baseline | performance baseline, documentation hygiene | — | phase15 to phase28 |
| phase30_release-v1-4-0 | release vehicle | — | phase14, phase15 to phase29 |

Changes from the suggested sequence: numbering starts at phase15 because
phase13 (archived, `ts-e2e-native-parity`) and phase14
(`replica-master-hostname`) were already taken. D-08 moved into phase21 with
N-01, since both come from per-transport persistence wiring, and its same-key
ordering part moved into phase23, which orders every write path at once. The
release task phase30 was added because the user asked for one version. No
priority was lowered.

## Decisions taken while writing the tasks

- Unclassified commands are denied in authenticated mode, enforced by a test
  over every dispatcher command name (phase16).
- In Hub mode the server refuses to start with RESP3 or SynapRPC enabled until
  those listeners gain tenant authentication (phase18).
- A failed WAL flush or fsync poisons the writer until restart (phase20).
- Invalid frames are classified by position: one with no complete frame after
  it is a torn tail and gets truncated; one followed by a valid frame is
  interior corruption and fails startup (phase20).
- The GUI configuration view becomes read-only over a new admin-only, redacted
  config command. No remote configuration writes are added (phase27).
- `Cargo.lock` is committed and nightly is pinned by date (phase28), following
  `.claude/rules/rust.md`, which says binaries commit their lockfile.

## Validation

`rulebook_task validate` passed for all 16 tasks with no errors. The only
warning is the expected one: tail items are unchecked. Every task is
`pending`, and no implementation item is checked. No code was changed, and no
acceptance test described in the tasks has been run.

## Coverage statement

All 21 finding IDs from the review (S-01 to S-07, D-01 to D-08, E-01 to E-06)
are covered by at least one task, as are the new finding N-01 and both
separate improvement proposals.

## Sequencing and blockers

- `phase10_typescript-7-swap` stays pending and externally blocked
  (typescript-eslint caps TypeScript below 6.1). The user had already set it
  aside. Sessions 11 to 13 proceeded past it on that basis, and this plan does
  the same. It is unrelated to the review work and is not part of 1.4.0 unless
  upstream unblocks it first.
- `phase14_replica-master-hostname` runs before phase15 and ships in 1.4.0.
- 1.3.2 still needs the user's authorization to push and tag. It is
  independent of this plan.
