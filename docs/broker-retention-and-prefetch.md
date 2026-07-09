# Broker Retention & Prefetch (Streams and Queues)

This guide documents two broker-parity behaviors added in the v1.0 hardening work
(audit findings M-012 and M-013): stream retention that does not silently lose
unread data, and per-consumer queue prefetch with fair dispatch.

## Streams: retention bounded by committed consumer offset

### The problem

A stream room keeps events in an in-memory ring buffer. Previously, on overflow
the oldest event was evicted regardless of whether any consumer had read it, so a
producer outrunning a slow consumer silently discarded unread events.

### The behavior

Retention is now bounded by the **minimum committed consumer offset** — the
position of the slowest tracked subscriber — with two size knobs:

- `max_buffer_size` — soft target. The buffer is trimmed back toward this size by
  evicting only events the slowest consumer has **already read**.
- `max_unread_buffer_size` — hard ceiling. Events not yet read by the slowest
  consumer are **protected**: the buffer is allowed to grow past `max_buffer_size`
  (up to this cap) rather than drop them. Only when it would exceed this cap is an
  unread event shed, and that real loss is counted in `RoomStats.dropped`.

Values below `max_buffer_size` are treated as `max_buffer_size` (plain ring
buffer). With **no subscribers** there is nothing to protect, so the room behaves
as a ring buffer at `max_buffer_size` regardless of the hard cap.

### The drop signal

`RoomStats.dropped` counts only evictions of events that were still unread by the
slowest tracked consumer. Normal recycling of already-consumed events is **not**
counted, so a non-zero `dropped` is an actionable lagging-consumer data-loss
signal rather than steady-state noise.

### Configuration

```yaml
# StreamConfig
max_buffer_size: 10000          # soft target per room
max_unread_buffer_size: 100000  # hard cap incl. unread (default 10x soft)
retention_secs: 3600
```

### Scope

`synap-core` is a leaf crate with no file I/O, so retention is bounded by the
committed offset rather than spilled to disk segments. A slow consumer therefore
gets a wide lag window (the hard cap) instead of silent loss, while memory stays
bounded even if it stalls forever. Kafka-style disk-segment durability remains
future work; the wire API is unchanged.

## Queues: per-consumer prefetch (QoS) and fair dispatch

### The problem

Queue consumption is pull-based (`consume(consumer_id)` pops one message). There
was no way to limit how many unacked messages a single consumer could hold, and
the reported consumer count was hardcoded to 1.

### The behavior

Each queue has a `prefetch_limit`: the maximum number of unacked (in-flight)
messages a single consumer may hold at once. A consumer already at its limit is
**not handed more messages until it acks**. In the pull model this is both:

- **QoS** — a greedy or slow consumer cannot monopolize the backlog, and
- **fair dispatch** — while one consumer is at its limit, pending messages remain
  available to other consumers instead of piling onto the fast one. `prefetch=1`
  yields the strictest fairness (one in-flight message per consumer), matching
  RabbitMQ's fair-dispatch pattern.

`prefetch_limit = 0` means unlimited (the default), preserving the previous
unthrottled behavior.

The reported consumer count (`QueueStats.consumers`) is now the number of distinct
consumers currently holding at least one in-flight message — a consumer holding N
messages counts once, and drops out on its last ack/nack/deadline-expiry.

### Configuration

Global default (server config):

```yaml
queue:
  prefetch_limit: 0   # 0 = unlimited (default)
```

Per-queue override at creation (REST / command):

```json
{ "prefetch_limit": 1 }
```

### Fairness note

Synap queues are pull-based, so there is no push scheduler to "round-robin".
Fairness is instead achieved through prefetch backpressure: throttling a consumer
at its limit lets its share flow to others. With `prefetch=1` this distributes
messages evenly across active consumers.
