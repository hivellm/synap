---
title: Monitoring
module: operations
id: monitoring
order: 2
description: Health checks, Prometheus metrics, and Grafana dashboards
tags: [monitoring, metrics, prometheus, grafana]
---

# Monitoring

Complete guide to monitoring Synap in production.

## Health Checks

### HTTP Health Endpoint

```bash
curl http://localhost:15500/health
```

**Response:**
```json
{
  "status": "healthy",
  "uptime_secs": 12345
}
```

### Server Info

```bash
curl http://localhost:15500/info
```

**Response:**
```json
{
  "version": "0.8.1",
  "uptime_secs": 12345,
  "memory_usage_bytes": 4294967296
}
```

## Prometheus Metrics

### Metrics Endpoint

```bash
curl http://localhost:15500/metrics
```

### Key Metrics

#### KV Store Metrics

```
# Total operations
synap_kv_operations_total{operation="get",status="success"} 1234
synap_kv_operations_total{operation="set",status="success"} 567

# Operation duration
synap_kv_operation_duration_seconds{operation="get"} 0.000087
synap_kv_operation_duration_seconds{operation="set"} 0.000123

# Memory usage
synap_kv_memory_bytes 8192
synap_kv_keys_total 42
```

#### Queue Metrics

```
# Queue depth
synap_queue_depth{queue="jobs"} 42

# Messages published
synap_queue_messages_published_total{queue="jobs"} 1000

# Messages consumed
synap_queue_messages_consumed_total{queue="jobs"} 955

# Dead letter queue
synap_queue_dlq_count{queue="jobs"} 5
```

#### Stream Metrics

```
# Stream messages
synap_stream_messages_total{stream="notifications"} 156

# Subscribers
synap_stream_subscribers{stream="notifications"} 3
```

#### System Metrics

```
# Process memory
synap_process_memory_bytes{type="used"} 4294967296

# CPU usage
synap_process_cpu_seconds_total 123.45

# Uptime
synap_process_uptime_seconds 12345
```

#### Replication Metrics

```
# Replication lag
synap_replication_lag_operations{role="replica"} 5
synap_replication_lag_ms{role="replica"} 10

# Replication status
synap_replication_connected{role="replica"} 1
```

## Prometheus Configuration

### prometheus.yml

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'synap'
    static_configs:
      - targets: ['localhost:15500']
    metrics_path: '/metrics'
    scrape_interval: 10s
```

### Docker Compose

```yaml
version: '3.8'
services:
  synap:
    image: hivellm/synap:latest
    ports:
      - "15500:15500"
  
  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    ports:
      - "9090:9090"
```

## Grafana Dashboards

### Import Dashboard

1. Go to Grafana → Dashboards → Import
2. Upload dashboard JSON or use dashboard ID
3. Configure Prometheus data source

### Key Panels

- **KV Operations**: Throughput, latency, error rate
- **Queue Depth**: Pending, in-flight, DLQ
- **Memory Usage**: Process memory, KV memory
- **Replication Lag**: Lag in operations and milliseconds
- **System Resources**: CPU, memory, disk

### Example Queries

```promql
# KV operations per second
rate(synap_kv_operations_total[5m])

# Average latency
avg(synap_kv_operation_duration_seconds)

# Queue depth
synap_queue_depth

# Memory usage
synap_process_memory_bytes

# Replication lag
synap_replication_lag_ms
```

## Alerting

### Alert Rules

```yaml
groups:
  - name: synap_alerts
    rules:
      - alert: HighMemoryUsage
        expr: synap_process_memory_bytes > 8589934592  # 8GB
        for: 5m
        annotations:
          summary: "High memory usage"
      
      - alert: HighQueueDepth
        expr: synap_queue_depth > 1000
        for: 5m
        annotations:
          summary: "High queue depth"
      
      - alert: HighReplicationLag
        expr: synap_replication_lag_ms > 10000
        for: 5m
        annotations:
          summary: "High replication lag"
      
      - alert: ServerDown
        expr: up{job="synap"} == 0
        for: 1m
        annotations:
          summary: "Synap server is down"
```

## Logging

### Log Levels

```yaml
logging:
  level: "info"  # trace, debug, info, warn, error
  format: "json"  # json, text
```

### View Logs

```bash
# Docker
docker logs synap -f

# Systemd
journalctl -u synap -f

# File
tail -f server.log
```

## Related Topics

- [Service Management](./SERVICE_MANAGEMENT.md) - Service management
- [Troubleshooting](./TROUBLESHOOTING.md) - Common problems
- [Log Management](./LOGS.md) - Log viewing and analysis

