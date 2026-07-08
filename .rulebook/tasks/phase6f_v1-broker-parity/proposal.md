# Proposal: phase6f_v1-broker-parity

Source: docs/analysis/synap-audit/ (M-012, M-013, M-016, M-017)

## Why
To claim parity with (let alone superiority over) Kafka and RabbitMQ, the streams and queues
subsystems need capabilities they currently lack. (M-012) Streams are RAM-only: on buffer
overflow `publish` pops the oldest event regardless of consumer progress
(`core/stream.rs:157-161`), silently dropping unread data — Kafka persists to disk segments and
retains by size/time independent of RAM. (M-016) `consume` linearly scans the whole buffer
filtering `offset >= from_offset` on every poll (`core/stream.rs:169-177`) — O(n) per read,
where an offset-indexed lookup is O(1) since offsets are contiguous. (M-013) Queues have no
per-consumer prefetch/QoS and the consumer count is hardcoded to 1 (`core/queue.rs:217`
`self.stats.consumers = 1; // Simplified for now`) — RabbitMQ's prefetch and fair dispatch are
core features. (M-017) The ACK-deadline sweep takes a single global write lock over all queues
once per second and scans every pending entry (`core/queue.rs:307-322,274-286`), stalling all
queue operations periodically at scale.

## What Changes
1. Streams: add optional disk-segment spill (or bound retention by the minimum committed
   consumer offset) so unread events are not silently dropped; surface an explicit drop/lag
   signal when retention forces a drop.
2. Streams: index consume by offset (buffer index = offset − min_offset) for O(1) seek instead
   of a full scan.
3. Queues: model consumers explicitly with a per-consumer prefetch window and fair round-robin
   dispatch; report the real consumer count.
4. Queues: replace the global 1 s deadline sweep with a per-queue min-heap/timer-wheel of ACK
   deadlines so expiry is O(expired) without a global lock.

## Impact
- Affected specs: stream retention + consume; queue consumer model + deadline handling (MODIFIED)
- Affected code: `crates/synap-core/src/stream.rs`, `core/queue.rs`,
  `core/consumer_group.rs`, stream persistence
- Breaking change: NO in wire API; new config knobs for stream spill and queue prefetch
- User benefit: streams stop losing unread data and scale reads; queues get fair multi-consumer
  dispatch and stop stalling periodically under load
