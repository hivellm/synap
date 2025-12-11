---
title: Performance Optimization
module: guides
id: performance-guide
order: 6
description: Advanced performance optimization techniques
tags: [guides, performance, optimization, tuning]
---

# Performance Optimization

Complete guide to optimizing Synap performance.

## Overview

Synap performance depends on:
- Memory configuration
- Persistence settings
- Network configuration
- System resources
- Application patterns

## Performance Targets

- **Read Latency**: < 1ms (87ns typical)
- **Write Latency**: < 1ms
- **Throughput**: 12M+ reads/sec, 44K+ writes/sec

## Memory Optimization

### Configure Memory Limits

```yaml
kv_store:
  max_memory_mb: 4096
  eviction_policy: "lru"
```

### Choose Eviction Policy

- **LRU**: Best for temporal locality
- **LFU**: Best for frequency-based access
- **None**: No eviction (use with caution)

### Monitor Memory Usage

```bash
curl http://localhost:15500/metrics | grep memory
```

## Persistence Tuning

### Fsync Mode Selection

| Mode | Throughput | Data Loss Risk | Use Case |
|------|------------|----------------|----------|
| always | ~1.7K ops/s | None | Critical data |
| periodic | ~44K ops/s | <10ms | Production (recommended) |
| never | ~44K ops/s | High | Non-critical, high throughput |

### Snapshot Frequency

- **High frequency** (5-15 min): Faster recovery, more I/O
- **Medium frequency** (1-6 hours): Balanced (recommended)
- **Low frequency** (daily): Slower recovery, less I/O

## Network Optimization

### Connection Pooling

```python
# Reuse connections
client = SynapClient("http://localhost:15500")

# Connection pool handles reuse automatically
```

### Batch Operations

```python
# Good: Batch operation
client.kv.mset([
    ("key1", "value1"),
    ("key2", "value2"),
    ("key3", "value3")
])

# Less efficient: Individual operations
client.kv.set("key1", "value1")
client.kv.set("key2", "value2")
client.kv.set("key3", "value3")
```

### Pipeline Requests

```python
# Pipeline multiple requests
pipeline = client.pipeline()
pipeline.set("key1", "value1")
pipeline.set("key2", "value2")
pipeline.set("key3", "value3")
results = pipeline.execute()
```

## Application Patterns

### Use Appropriate Data Structures

```python
# Good: Use hash for objects
client.hash.hset("user:1", "name", "John")
client.hash.hset("user:1", "age", "30")

# Less efficient: Multiple keys
client.kv.set("user:1:name", "John")
client.kv.set("user:1:age", "30")
```

### Minimize Round-Trips

```python
# Good: Single round-trip
values = client.kv.mget(["key1", "key2", "key3"])

# Less efficient: Multiple round-trips
value1 = client.kv.get("key1")
value2 = client.kv.get("key2")
value3 = client.kv.get("key3")
```

### Use TTL Appropriately

```python
# Set TTL for temporary data
client.kv.set("session:abc", data, ttl=3600)

# Prevents memory bloat
```

## System Resources

### CPU

- **Single core**: ~12M reads/sec, ~44K writes/sec
- **Multiple cores**: Scales linearly
- **Recommendation**: 2-4 cores minimum

### Memory

- **Minimum**: 512MB
- **Recommended**: 2-4GB
- **High performance**: 8GB+

### Disk

- **SSD recommended**: For WAL and snapshots
- **Network storage**: OK for snapshots (not WAL)
- **Local storage**: Best for WAL

## Monitoring Performance

### Key Metrics

```bash
# Operation throughput
curl http://localhost:15500/metrics | grep operations_total

# Latency
curl http://localhost:15500/metrics | grep duration_seconds

# Memory usage
curl http://localhost:15500/metrics | grep memory_bytes
```

### Performance Profiling

```bash
# Enable debug logging
export RUST_LOG=debug

# Monitor slow operations
curl http://localhost:15500/metrics | grep duration_seconds | grep -v "0\."
```

## Best Practices

### Production Settings

```yaml
server:
  host: "0.0.0.0"
  port: 15500

kv_store:
  max_memory_mb: 4096
  eviction_policy: "lru"

persistence:
  enabled: true
  wal:
    enabled: true
    fsync_mode: "periodic"
    fsync_interval_ms: 10
  snapshot:
    enabled: true
    interval_secs: 3600
```

### High Performance Settings

```yaml
kv_store:
  max_memory_mb: 8192
  eviction_policy: "lru"

persistence:
  wal:
    fsync_mode: "periodic"
    fsync_interval_ms: 5  # More frequent fsync
```

## Related Topics

- [Performance Tuning](../configuration/PERFORMANCE_TUNING.md) - Configuration tuning
- [Monitoring Guide](../operations/MONITORING.md) - Monitoring and metrics

