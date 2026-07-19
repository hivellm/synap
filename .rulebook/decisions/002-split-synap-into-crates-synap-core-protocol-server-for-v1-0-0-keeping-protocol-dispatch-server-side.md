# 2. Split Synap into crates/synap-{core,protocol,server} for v1.0.0, keeping protocol dispatch server-side

**Status**: proposed
**Date**: 2026-07-08
**Related Tasks**: phase2_v1-workspace-skeleton, phase3_v1-extract-synap-protocol, phase4_v1-extract-synap-core, phase5_v1-wire-rust-sdk-to-protocol

## Context

Synap is the last HiveLLM store on a flat workspace (synap-server is a ~60K-LOC monolith with 23 modules). Vectorizer and Nexus already use the crates/ layout (name-core/protocol/server/cli). Analysis synap-v1-release found that Synap's protocol/ module is not pure wire code: resp3/command/ and synap_rpc/dispatch/ depend on AppState, crate::core and crate::scripting, so a wholesale move would create a protocol→server dependency cycle.

## Decision

Adopt the Vectorizer/Nexus crates/ layout for v1.0.0 in incremental phases: (1) pure directory move with zero refactor, (2) extract synap-protocol containing ONLY wire code (envelope, RESP3 parser/writer + value type, SynapRPC codec/types), (3) extract synap-core as a leaf crate (core/, cache/, compression/, simd/), (4) wire sdks/rust to synap-protocol. The RESP3 command layer and SynapRPC dispatch layer stay inside synap-server permanently — they are request handlers bound to AppState, not wire code.

## Alternatives Considered

- Move protocol/ wholesale into synap-protocol — rejected: creates a protocol→server cycle via AppState/core/scripting imports
- Big-bang restructure in one phase — rejected: Vectorizer precedent shows incremental sub-phases with per-phase cargo gates are reviewable and low-risk
- Keep the flat layout for 1.0 — rejected: inconsistent with Vectorizer/Nexus, blocks SDK wire-type de-duplication and slows incremental builds

## Consequences

Breaking import paths for Rust consumers of synap_server::core::*/protocol::* (mitigated by umbrella re-exports + CHANGELOG migration guide in phase 8). Faster incremental builds and an independently benchmarkable engine. sdks/rust gains a single source of truth for wire types. Import rewrite in phase 4 touches nearly every server file — executed one module per commit with cargo check after each.
