---
title: Performance Tuning
module: configuration
id: performance-tuning
order: 6
description: Performance optimization and tuning
tags: [configuration, performance, optimization, tuning]
---

# Performance Tuning

Complete guide to optimizing Synap performance.

## Overview

Synap performance depends on:
- Memory configuration
- Persistence settings
- Network configuration
- System resources

## Memory Configuration

### KV Store Memory Limits

```yaml
kv_store:
  max_memory_mb: 4096  # 4GB limit
  eviction_policy: "lru"  # lru, lfu, or none
```

### Eviction Policies

#### LRU (Least Recently Used)

```yaml
kv_store:
  eviction_policy: "lru"
```

Evicts least recently used keys when memory limit reached.

#### LFU (Least Frequently Used)

```yaml
kv_store:
  eviction_policy: "lfu"
```

Evicts least frequently used keys when memory limit reached.

#### None

```yaml
kv_store:
  eviction_policy: "none"
```

No eviction (memory can grow unbounded).

## Persistence Tuning

### Fsync Mode

#### Maximum Performance

```yaml
persistence:
  wal:
    fsync_mode: "never"  # ~44K ops/s
```

#### Balanced (Recommended)

```yaml
persistence:
  wal:
    fsync_mode: "periodic"
    fsync_interval_ms: 10  # ~44K ops/s
```

#### Maximum Safety

```yaml
persistence:
  wal:
    fsync_mode: "always"  # ~1.7K ops/s
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

## Network Tuning

### Connection Limits

Configure in reverse proxy:

```nginx
upstream synap {
    server localhost:15500;
    keepalive 100;
}

server {
    keepalive_timeout 65;
    keepalive_requests 1000;
}
```

### Timeouts

```yaml
server:
  read_timeout_secs: 30
  write_timeout_secs: 30
```

## Optimization Tips

### Use Batch Operations

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

### Connection Pooling

```python
# Reuse connections
client = SynapClient("http://localhost:15500")

# Use connection pool
# (SDK handles this automatically)
```

### Compression

For large values, compress before storing:

```python
import gzip
import json

data = {"large": "..." * 1000}
compressed = gzip.compress(json.dumps(data).encode())
client.kv.set("key", compressed)
```

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

### Performance Targets

- **Read latency**: < 1ms (87ns typical)
- **Write latency**: < 1ms
- **Throughput**: 12M+ reads/sec, 44K+ writes/sec

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

replication:
  enabled: true
  role: "master"
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

### Development Settings

```yaml
kv_store:
  max_memory_mb: 1024
  eviction_policy: "none"  # No eviction for dev

persistence:
  wal:
    fsync_mode: "never"  # Faster for dev
```

## Related Topics

- [Configuration Overview](./CONFIGURATION.md) - General configuration
- [Persistence Configuration](./PERSISTENCE.md) - WAL and snapshots
- [Monitoring Guide](../operations/MONITORING.md) - Monitoring and metrics

