# 6. Ship the next-release review hardening as one 1.4.0 release

**Status**: proposed
**Date**: 2026-10-13
**Related Tasks**: phase30_release-v1-4-0

## Context

The 2026-10-12 next-release review (docs/analysis/next-release-review-2026-10-12/) found 21 defects in authorization, durability and the client ecosystem; turning it into tasks added N-01 (RESP3/SynapRPC writes never reach the WAL or replicas). The fixes change authorization semantics on every transport, the WAL and snapshot formats, and Hub mode startup rules. The user asked for all of it to ship in a single version.

## Decision

All review work (phase15 to phase29) ships together as 1.4.0, released by phase30; phase14 lands in the same release. Minor, not major: 1.4.0 reads 1.3.x WALs and v3 snapshots, and the wire protocols are unchanged. Downgrading after 1.4.0 has written data is unsupported and the migration guide says so with a backup step. Cross-cutting choices fixed here: one OperationContext and command classifier authorizes every transport (fail closed on unclassified commands); one mutation path persists every write; Hub mode refuses to start with RESP3/SynapRPC enabled until they gain tenant auth; the GUI configuration view becomes read-only over a redacted server config command instead of gaining remote config writes; Cargo.lock is committed and nightly is pinned by date.

## Alternatives Considered

- Fold the work into the unpushed 1.3.2: rejected, 1.3.2 is a finished dependency/security release and would be held for months
- Split into a 1.3.3 patch (security) and a 1.4.0 (format changes): rejected by the user, who asked for one version
- Call it 2.0.0: rejected, old data stays readable and no wire format changes

## Consequences

Deployments that relied on transport ACL gaps, unenforced rate limits, or RESP3/SynapRPC in Hub mode see failures after upgrading and must follow the migration guide. Release 1.4.0 waits for every task. Native writes gain WAL and replication cost, which phase29 measures.
