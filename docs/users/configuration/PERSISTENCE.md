---
title: Persistence Configuration
module: configuration
id: persistence-configuration
order: 4
description: WAL, snapshots, and durability configuration
tags: [configuration, persistence, wal, snapshots, durability]
---

# Persistence Configuration

Complete guide to configuring persistence, WAL, and snapshots in Synap.

## Overview

Synap provides two persistence mechanisms:
- **WAL (Write-Ahead Log)**: Logs all writes for recovery
- **Snapshots**: Periodic full state snapshots

## Basic Configuration

### Enable Persistence

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
  snapshot:
    enabled: true
    directory: "./data/snapshots"
```

## WAL Configuration

### Basic WAL

```yaml
persistence:
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
```

### Fsync Modes

#### Always (Safest)

```yaml
persistence:
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
    fsync_mode: "always"
```

- Fsync every write
- Safest, slowest (~1.7K ops/s)
- Zero data loss on crash

#### Periodic (Recommended)

```yaml
persistence:
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
    fsync_mode: "periodic"
    fsync_interval_ms: 10
```

- Fsync every 10ms
- Balanced (~44K ops/s)
- Minimal data loss (up to 10ms)

#### Never (Fastest)

```yaml
persistence:
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
    fsync_mode: "never"
```

- OS handles fsync
- Fastest (~44K ops/s)
- Risk of data loss on crash

### WAL Size Limits

```yaml
persistence:
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
    max_size_mb: 1024  # 1GB max
    rotate_on_size: true
```

## Snapshot Configuration

### Basic Snapshots

```yaml
persistence:
  snapshot:
    enabled: true
    directory: "./data/snapshots"
    interval_secs: 3600  # Every hour
    auto_snapshot: true
```

### Snapshot Settings

```yaml
persistence:
  snapshot:
    enabled: true
    directory: "./data/snapshots"
    interval_secs: 3600
    auto_snapshot: true
    compression: true  # Compress snapshots
    keep_count: 7  # Keep last 7 snapshots
```

## Recovery

### Automatic Recovery

Synap automatically recovers on startup:

1. Load latest snapshot
2. Replay WAL from snapshot offset
3. Server ready

### Recovery Time

- **1M keys**: ~1-5 seconds
- **10M keys**: ~10-30 seconds
- **100M keys**: ~1-5 minutes

## Manual Operations

### Create Snapshot

```bash
curl -X POST http://localhost:15500/snapshot
```

**Response:**
```json
{
  "success": true,
  "snapshot_path": "./data/snapshots/snapshot-v2-1234567890.bin"
}
```

### List Snapshots

```bash
curl http://localhost:15500/snapshots
```

**Response:**
```json
{
  "snapshots": [
    {
      "path": "snapshot-v2-1234567890.bin",
      "size_bytes": 1048576,
      "created_at": "2025-01-01T12:00:00Z"
    }
  ]
}
```

## Performance Considerations

### Fsync Mode Selection

| Mode | Throughput | Data Loss Risk | Use Case |
|------|------------|----------------|----------|
| always | ~1.7K ops/s | None | Critical data |
| periodic | ~44K ops/s | <10ms | Production (recommended) |
| never | ~44K ops/s | High | Non-critical, high throughput |

### Snapshot Frequency

- **High frequency** (every 5-15 min): Faster recovery, more I/O
- **Medium frequency** (every 1-6 hours): Balanced (recommended)
- **Low frequency** (daily): Slower recovery, less I/O

### Disk Requirements

- **WAL**: ~10-100MB typical
- **Snapshots**: Varies by data size
- **Total**: 2-3x data size (with snapshots)

## Best Practices

### Production Settings

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
    path: "/data/wal/synap.wal"
    fsync_mode: "periodic"
    fsync_interval_ms: 10
  snapshot:
    enabled: true
    directory: "/data/snapshots"
    interval_secs: 3600
    auto_snapshot: true
    keep_count: 7
```

### Development Settings

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
    fsync_mode: "never"  # Faster for development
  snapshot:
    enabled: false  # Disable for development
```

### High Availability

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
    path: "/data/wal/synap.wal"
    fsync_mode: "always"  # Maximum safety
  snapshot:
    enabled: true
    directory: "/data/snapshots"
    interval_secs: 900  # Every 15 minutes
    keep_count: 24  # Keep 24 snapshots (6 hours)
```

## Monitoring

### Check Persistence Status

```bash
curl http://localhost:15500/info
```

**Response:**
```json
{
  "version": "0.8.1",
  "persistence": {
    "enabled": true,
    "wal_size_bytes": 1048576,
    "snapshot_count": 7
  }
}
```

### Monitor WAL Size

```bash
# Check WAL file size
ls -lh data/wal/synap.wal

# Check via metrics
curl http://localhost:15500/metrics | grep wal
```

## Related Topics

- [Configuration Overview](./CONFIGURATION.md) - General configuration
- [Replication Configuration](./REPLICATION.md) - Master-replica setup
- [Performance Tuning](./PERFORMANCE_TUNING.md) - Performance optimization

