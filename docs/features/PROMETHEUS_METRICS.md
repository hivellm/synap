# Prometheus Metrics for Synap

## Overview

Synap exposes comprehensive Prometheus metrics for monitoring all system components.

**Endpoint**: `GET /metrics`  
**Format**: Prometheus text format (version 0.0.4)  
**Status**: ✅ Production Ready

## Available Metrics

### KV Store Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `synap_kv_operations_total` | Counter | `operation`, `status` | Total KV operations |
| `synap_kv_operation_duration_seconds` | Histogram | `operation` | KV operation latency |
| `synap_kv_keys_total` | Gauge | `shard` | Current number of keys |
| `synap_kv_memory_bytes` | Gauge | `type` | Memory usage in bytes |

**Operations**: `get`, `set`, `delete`, `scan`, `mget`, `mset`, `mdel`  
**Status**: `success`, `error`

### Queue Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `synap_queue_operations_total` | Counter | `queue`, `operation`, `status` | Total queue operations |
| `synap_queue_operation_duration_seconds` | Histogram | `queue`, `operation` | Queue operation latency |
| `synap_queue_depth` | Gauge | `queue` | Pending messages |
| `synap_queue_dlq_messages` | Gauge | `queue` | Messages in DLQ |

**Operations**: `publish`, `consume`, `ack`, `nack`, `purge`

### Stream Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `synap_stream_operations_total` | Counter | `room`, `operation`, `status` | Total stream operations |
| `synap_stream_events_total` | Counter | `room`, `event_type` | Events published |
| `synap_stream_subscribers` | Gauge | `room` | Active subscribers |
| `synap_stream_buffer_size` | Gauge | `room` | Events in buffer |

### Pub/Sub Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `synap_pubsub_operations_total` | Counter | `operation`, `status` | Total pub/sub operations |
| `synap_pubsub_messages_total` | Counter | `topic` | Messages published |
| `synap_pubsub_subscriptions` | Gauge | `topic` | Active subscriptions |

### Replication Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `synap_replication_lag_operations` | Gauge | `replica_id` | Replication lag (ops) |
| `synap_replication_operations_total` | Counter | `type`, `status` | Replication operations |
| `synap_replication_bytes_total` | Counter | `direction` | Bytes transferred |

**Types**: `full_sync`, `partial_sync`, `append`  
**Direction**: `sent`, `received`

### HTTP Server Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `synap_http_requests_total` | Counter | `method`, `path`, `status` | Total HTTP requests |
| `synap_http_request_duration_seconds` | Histogram | `method`, `path` | HTTP request latency |
| `synap_http_connections` | Gauge | `type` | Active connections |

### System Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `synap_process_memory_bytes` | Gauge | `type` | Process memory usage |
| `synap_process_cpu_usage_percent` | Gauge | `core` | CPU usage percentage |

## Usage Examples

### Prometheus Configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'synap'
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:15500']
    metrics_path: '/metrics'
```

### Grafana Queries

**KV Operations Rate**:
```promql
rate(synap_kv_operations_total[5m])
```

**Queue Depth**:
```promql
synap_queue_depth
```

**P95 Latency**:
```promql
histogram_quantile(0.95, rate(synap_kv_operation_duration_seconds_bucket[5m]))
```

**Replication Lag**:
```promql
synap_replication_lag_operations
```

**Error Rate**:
```promql
rate(synap_kv_operations_total{status="error"}[5m])
```

## Integration

### Automatic Recording

Metrics are automatically recorded by:
- `handlers.rs` - HTTP request metrics
- `kv_store.rs` - KV operation metrics
- `queue.rs` - Queue metrics
- `stream.rs` - Stream metrics
- `replication/master.rs` - Replication metrics

### Manual Recording

```rust
use synap_server::metrics;

// Record KV operation
metrics::record_kv_op("get", "success", 0.001);

// Record queue operation
metrics::record_queue_op("my-queue", "publish", "success", 0.005);

// Update replication lag
metrics::update_replication_lag("replica-1", 150);
```

## Grafana Dashboard

Import the provided dashboard JSON:

```bash
# Download dashboard
curl -O https://raw.githubusercontent.com/hivellm/synap/main/docs/grafana/synap-dashboard.json

# Import to Grafana via UI or API
curl -X POST http://localhost:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -d @synap-dashboard.json
```

## Alert Rules

### Recommended Prometheus Alerts

```yaml
groups:
  - name: synap
    rules:
      # High error rate
      - alert: SynapHighErrorRate
        expr: rate(synap_kv_operations_total{status="error"}[5m]) > 100
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate in Synap KV operations"

      # Queue depth too high
      - alert: SynapQueueDepthHigh
        expr: synap_queue_depth > 10000
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Queue {{ $labels.queue }} has {{ $value }} pending messages"

      # Replication lag
      - alert: SynapReplicationLag
        expr: synap_replication_lag_operations > 1000
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Replica {{ $labels.replica_id }} is {{ $value }} operations behind"

      # Memory usage
      - alert: SynapHighMemory
        expr: synap_process_memory_bytes{type="used"} > 8e9  # 8GB
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Synap memory usage is {{ $value | humanize }}B"
```

## Performance Impact

- **Overhead**: < 1% CPU
- **Memory**: ~2MB for metric storage
- **Latency**: < 1µs per metric recording

## Best Practices

1. **Scrape Interval**: 15-30 seconds
2. **Retention**: 15-30 days
3. **Cardinality**: Monitor label cardinality (avoid high-cardinality labels)
4. **Aggregation**: Use recording rules for heavy queries

---

**Status**: ✅ Production Ready  
**Last Updated**: October 22, 2025  
**Version**: Synap v0.3.0


