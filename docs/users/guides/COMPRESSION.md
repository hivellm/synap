---
title: Compression Guide
module: guides
id: compression
order: 11
description: Data compression and optimization
tags: [guides, compression, optimization, performance]
---

# Compression Guide

Learn how Synap uses compression to optimize storage and network performance.

## Overview

Synap supports automatic compression for:

- **Persistence** - Compress data in WAL and snapshots
- **Network** - Compress HTTP responses (gzip)
- **Storage** - Reduce disk usage for persisted data

## Compression Algorithms

### Supported Algorithms

1. **LZ4** - Fast compression, good balance
2. **Zstd** - Better compression ratio, slightly slower
3. **Gzip** - Standard HTTP compression

### Algorithm Comparison

| Algorithm | Speed | Ratio | Use Case |
|-----------|-------|-------|----------|
| LZ4 | ⚡⚡⚡ Fast | Good | Real-time, low latency |
| Zstd | ⚡⚡ Fast | Better | Storage, persistence |
| Gzip | ⚡ Standard | Good | HTTP responses |

## Configuration

### Enable Compression

```yaml
persistence:
  enabled: true
  compression:
    algorithm: "zstd"  # Options: lz4, zstd, none
    level: 3           # Compression level (1-22 for zstd, 1-9 for lz4)
```

### Compression Levels

**LZ4:**
- Level 1-9 (default: 1)
- Higher = better compression, slower

**Zstd:**
- Level 1-22 (default: 3)
- Level 1 = fastest
- Level 22 = best compression

### Recommended Settings

**High Performance:**
```yaml
persistence:
  compression:
    algorithm: "lz4"
    level: 1
```

**Balanced:**
```yaml
persistence:
  compression:
    algorithm: "zstd"
    level: 3
```

**Maximum Compression:**
```yaml
persistence:
  compression:
    algorithm: "zstd"
    level: 10
```

## HTTP Compression

### Automatic Gzip Compression

Synap automatically compresses HTTP responses:

```bash
# Request with compression
curl -H "Accept-Encoding: gzip" http://localhost:15500/kv/get/my-key

# Response is automatically compressed
```

### Compression Headers

**Request:**
```
Accept-Encoding: gzip, deflate
```

**Response:**
```
Content-Encoding: gzip
Content-Length: 1234
```

## Persistence Compression

### WAL Compression

Compress write-ahead log entries:

```yaml
persistence:
  wal:
    compression:
      algorithm: "lz4"
      level: 1
```

**Benefits:**
- Reduced disk I/O
- Faster writes (less data to write)
- Lower disk usage

### Snapshot Compression

Compress snapshot files:

```yaml
persistence:
  snapshots:
    compression:
      algorithm: "zstd"
      level: 6
```

**Benefits:**
- Smaller snapshot files
- Faster backup/restore
- Lower storage costs

## Performance Impact

### Compression Overhead

**CPU Usage:**
- LZ4: ~5-10% CPU overhead
- Zstd: ~10-20% CPU overhead

**Latency:**
- LZ4: <1ms additional latency
- Zstd: 1-3ms additional latency

**Storage Savings:**
- LZ4: 30-50% reduction
- Zstd: 50-70% reduction

### When to Use Compression

**Use Compression When:**
- ✅ Disk space is limited
- ✅ Network bandwidth is limited
- ✅ Data is compressible (text, JSON, etc.)
- ✅ CPU is available

**Skip Compression When:**
- ❌ Data is already compressed (images, videos)
- ❌ CPU is constrained
- ❌ Latency is critical (<1ms requirements)
- ❌ Data is small (<1KB)

## Best Practices

### 1. Choose Right Algorithm

**For Real-Time:**
```yaml
compression:
  algorithm: "lz4"
  level: 1
```

**For Storage:**
```yaml
compression:
  algorithm: "zstd"
  level: 6
```

### 2. Monitor Compression Ratio

```python
from synap import SynapClient

client = SynapClient("http://localhost:15500")

# Get statistics
stats = client.info()

print(f"Compression ratio: {stats['compression_ratio']}")
print(f"Disk usage: {stats['disk_usage']}")
```

### 3. Test Different Levels

**Benchmark:**
```bash
# Test with different compression levels
for level in 1 3 6 9; do
  echo "Testing level $level"
  # Run benchmark
done
```

### 4. Balance Compression and Performance

**Start Conservative:**
```yaml
compression:
  algorithm: "lz4"
  level: 1
```

**Optimize Based on Results:**
- Monitor CPU usage
- Monitor disk usage
- Adjust level based on needs

## Use Cases

### 1. High-Volume Logging

Compress log entries in WAL:

```yaml
persistence:
  wal:
    compression:
      algorithm: "lz4"
      level: 1
```

### 2. Backup Optimization

Compress snapshots for backups:

```yaml
persistence:
  snapshots:
    compression:
      algorithm: "zstd"
      level: 10
```

### 3. Network Optimization

Enable HTTP compression for API responses:

```yaml
server:
  compression:
    enabled: true
    algorithm: "gzip"
```

## Troubleshooting

### High CPU Usage

**Problem:** Compression causing high CPU usage.

**Solution:**
1. Use LZ4 instead of Zstd
2. Lower compression level
3. Disable compression for small values

### Slow Writes

**Problem:** Compression slowing down writes.

**Solution:**
1. Use LZ4 (faster)
2. Lower compression level
3. Consider disabling for high-throughput scenarios

### Poor Compression Ratio

**Problem:** Compression not reducing size much.

**Solution:**
1. Data may already be compressed
2. Try higher compression level
3. Check data type (binary data compresses less)

## Related Topics

- [Persistence Guide](./PERSISTENCE.md) - Persistence configuration
- [Performance Guide](./PERFORMANCE.md) - Performance optimization
- [Configuration Guide](../configuration/CONFIGURATION.md) - Complete configuration

