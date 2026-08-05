# Wall-clock-seeded generation id discriminates ephemeral-state wipes

**Category**: architecture
**Tags**: streams, stats, consumer-cursor, restart-safety, issue-257

## Description

When a server hands clients a cursor into ephemeral state (stream offsets, queue
positions, snapshot indexes), the client needs a way to notice that the state was
destroyed and rebuilt underneath it. Counters cannot do that job: `message_count`,
`max_offset`, `total_published` and friends all reset to zero on the wipe, and any of
them can coincide with a value the client last observed — that coincidence is exactly
when a stale cursor silently skips data (issue #257 hit it with N=1).

The fix is an identity for the incarnation, not another counter. Stamp each room at
construction with a `generation` drawn from a process-global source that is (a) strictly
increasing and (b) seeded from the wall clock:

```rust
static ROOM_GENERATION: AtomicU64 = AtomicU64::new(0);
// candidate = now_millis().max(previous + 1), installed with compare_exchange_weak
```

Strict monotonicity handles many rooms created inside one millisecond; the wall-clock
seed handles the restart case, because a fresh process starts from "now", which is past
every id the previous process could have emitted — no persisted state, no coordination.
Ship `created_at` next to it for human debugging.

Clients then hold `(cursor, generation)` and rewind whenever the generation changes.
Expose it on **every** transport that serves stats (`crates/synap-core/src/core/stream.rs`
plus the RESP3 and SynapRPC `SSTATS` maps), otherwise consumers on the native wires keep
the unfixable version of the problem.
