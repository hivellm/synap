# Observability — Prometheus metrics

Synap exposes Prometheus metrics at `GET /metrics` (always public, no auth).
The endpoint refreshes system and broker gauges on every scrape, so the values
reflect live state at scrape time.

## Broker-state gauges

These are populated from a fresh snapshot of the broker on each scrape, and are
**reset between scrapes** — a stream/group/queue that has been deleted stops
reporting immediately rather than freezing at its last value.

### Streams / rooms

| Metric | Labels | Meaning |
|--------|--------|---------|
| `synap_stream_buffer_size` | `room` | Events currently buffered in the room |
| `synap_stream_last_offset` | `room` | Last (highest) offset published to the room |
| `synap_stream_subscribers` | `room` | Active subscribers on the room |

### Partitioned topics

| Metric | Labels | Meaning |
|--------|--------|---------|
| `synap_partition_messages` | `topic`, `partition` | Events buffered in the partition |
| `synap_partition_end_offset` | `topic`, `partition` | High-water-mark (last published offset) |

### Consumer groups

| Metric | Labels | Meaning |
|--------|--------|---------|
| `synap_consumer_group_members` | `group`, `topic` | Active members in the group |
| `synap_consumer_group_committed_offset` | `group`, `topic`, `partition` | Last committed (acked) offset |
| `synap_consumer_group_lag` | `group`, `topic`, `partition` | `end_offset − committed_offset`, clamped ≥ 0 |

**Consumer lag** is the primary signal for a stuck or slow consumer group:

```promql
# Total un-consumed backlog for a group across all partitions
sum by (group) (synap_consumer_group_lag{group="cortex-embedder"})

# Alert: lag climbing while committed offset is flat ⇒ stuck consumer
max by (group) (synap_consumer_group_lag) > 10000
```

### Queues

| Metric | Labels | Meaning |
|--------|--------|---------|
| `synap_queue_depth` | `queue` | Ready (undelivered) messages |
| `synap_queue_dlq_messages` | `queue` | Messages dead-lettered |

## System gauges — process vs. host

> **Note (changed in 0.13.0):** the `synap_process_*` gauges now measure **this
> Synap process**. Previously they reported host-wide values —
> `synap_process_cpu_usage_percent` was the host **load average** and
> `synap_process_memory_bytes` was host memory — which made an idle broker on a
> busy shared host look like it was burning CPU. Host stats are still available,
> under the correctly-named `synap_host_*` gauges.

| Metric | Labels | Meaning |
|--------|--------|---------|
| `synap_process_cpu_usage_percent` | `core="process"` | CPU% of the Synap process (100 = one core) |
| `synap_process_memory_bytes` | `type="rss"` / `"virtual"` | Resident / virtual memory of the process |
| `synap_host_memory_bytes` | `type="used"` / `"total"` | Whole-machine memory |
| `synap_host_load_average` | `window="1min"` / `"5min"` / `"15min"` | Host load average × 100 |

Process CPU is sampled with [`sysinfo`](https://crates.io/crates/sysinfo): the
sampler is kept across scrapes so `cpu_usage()` is the average over the interval
between the two most recent scrapes (the first scrape after start reports 0).

## Other metric families

The endpoint also exposes KV, pub/sub, HTTP, RESP3, SynapRPC, and replication
counters/histograms — see `synap-server/src/metrics/mod.rs` for the full list.
