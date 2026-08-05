# Proposal: phase1_stream-room-generation

Source: https://github.com/hivellm/synap/issues/257

## Why

Stream rooms are ephemeral: a server restart wipes them and the upstream
publisher lazily recreates them. Every field `stream.stats` returns today
(`message_count`, `min_offset`, `max_offset`, `total_published`,
`total_consumed`, `subscriber_count`, `dropped`) resets on that wipe, so a
long-lived consumer that tracks its own `from_offset` cursor cannot tell a
healthy caught-up room from a wiped-and-refilled one whose new event count
happens to match its cursor. In that coincidence the consumer silently skips
every post-wipe event below its cursor — observed live with N=1 in the Cortex
hive-services battery: cursor 1 == post-wipe head 1, the `cursor > head`
stale-heal never fires, and the offset-0 event is never delivered.

Heuristics cannot close the gap: `total_consumed` is inflated by other
subscribers on shared rooms, and remembering `total_published` to detect a
decrease fails exactly when the pre-wipe and post-wipe counts coincide — the
same coincidence that breaks the cursor comparison. The room needs an identity
that does not reset to a previously observed value.

## What Changes

- `RoomStats` gains two discriminators, set when the room is (re)created:
  - `created_at` — epoch milliseconds of the room's creation;
  - `generation` — a strictly monotonic id, seeded from the wall clock so it
    never repeats a value from a previous server process.
- Both fields are exposed on every transport that serves room stats: REST
  `GET /stream/:room/stats`, the StreamableHTTP `stream.stats` command, RESP3
  `SSTATS` and SynapRPC `SSTATS`.
- The Rust SDK's `StreamStats` and the TypeScript `StreamStats` interface carry
  the new fields. The TypeScript interface is realigned with the actual server
  payload at the same time — it currently declares `subscribers`,
  `total_events`, `room` and `last_activity`, none of which the server has ever
  sent, so those fields are always `undefined` at runtime.
- Python/PHP/C# return the stats payload untyped (dict/array/Dictionary), so
  the new fields flow through without SDK changes; the user docs are updated.

## Impact

- Affected specs: `.rulebook/tasks/phase1_stream-room-generation/specs/stream/spec.md`
- Affected code: `crates/synap-core/src/core/stream.rs`,
  `crates/synap-server/src/protocol/resp3/command/advanced.rs`,
  `crates/synap-server/src/protocol/synap_rpc/dispatch/advanced.rs`,
  `sdks/rust/src/types.rs`, `sdks/typescript/src/types.ts`,
  `docs/users/streams/CONSUMING.md`, `docs/api/REST_API.md`
- Breaking change: NO for the server wire (both fields are additive). The
  TypeScript `StreamStats` interface drops four fields that never existed in
  the payload — a type-level fix, not a runtime behavior change.
- User benefit: a consumer detects a room wipe by comparing `generation`
  against the one it remembered, and rewinds unambiguously regardless of any
  count coincidence.
