# phase6f design: committed-offset stream retention + prefetch-as-fair-dispatch
**Source**: manual
**Date**: 2026-07-09
**Related Task**: phase6f_v1-broker-parity
**Tags**: analysis:synap-audit, phase6f, streams, queues, architecture, leaf-crate
Two architectural choices when implementing phase6f broker-parity in synap-core (a LEAF crate with no file I/O):

1. Stream retention (M-012, tasks 1.1/1.7): chose "bound retention by min committed consumer offset" over Kafka-style disk-segment spill, because synap-core is pure in-memory data structures and adding file I/O there breaks the layering (disk spill would require a cross-crate callback into synap-server persistence — much larger). Design: two knobs on StreamConfig — max_buffer_size (soft: evict only events the slowest subscriber already read) and max_unread_buffer_size (hard cap, default 10x soft = 100k). Unread events (offset >= min subscriber last_offset) are protected; the buffer grows up to the hard cap rather than dropping them. Only a forced drop at the hard cap increments RoomStats.dropped. No subscribers => nothing to protect => plain ring buffer. Publish loop peeks front, breaks when oldest is unread AND len <= hard_cap. crates/synap-core/src/core/stream.rs Room::publish (commit 623313f).

2. Queue prefetch/fair-dispatch (M-013, tasks 1.4/1.5): Synap queues are PULL-based (consume(consumer_id) pops one), so there is no push scheduler to "round-robin". Insight: a per-queue prefetch_limit enforced against the M-013 in-flight active_consumers map gives BOTH QoS (throttle a greedy consumer: return None from consume when in_flight >= limit) AND fair dispatch (a throttled consumer's share stays available to others). prefetch=1 == RabbitMQ strict fair dispatch. prefetch_limit=0 = unlimited (default, back-compat). Wired through server QueueSystemConfig.prefetch_limit (global) + CreateQueueRequest.prefetch_limit (per-queue override). crates/synap-core/src/core/queue.rs Queue::consume (commit 90ac3c9).

Both fully close phase6f (all 8 impl items + tail); full workspace suite 1708 passed. Remaining broker durability (Kafka-style on-disk segments) is explicitly future work, not a 1.0 blocker.