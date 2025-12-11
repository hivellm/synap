---
title: Persistence
module: guides
id: persistence-guide
order: 2
description: WAL and snapshots for data durability
tags: [guides, persistence, wal, snapshots, durability]
---

# Persistence

Complete guide to WAL and snapshots in Synap.

## Overview

Synap provides two persistence mechanisms:
- **WAL (Write-Ahead Log)**: Logs all writes for recovery
- **Snapshots**: Periodic full state snapshots

## WAL (Write-Ahead Log)

### Configuration

```yaml
persistence:
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
    fsync_mode: "periodic"  # always, periodic, never
    fsync_interval_ms: 10
```

### Fsync Modes

#### Always (Safest)

```yaml
wal:
  fsync_mode: "always"
```

- Fsync every write
- Safest, slowest (~1.7K ops/s)
- Zero data loss on crash

#### Periodic (Recommended)

```yaml
wal:
  fsync_mode: "periodic"
  fsync_interval_ms: 10
```

- Fsync every 10ms
- Balanced (~44K ops/s)
- Minimal data loss (up to 10ms)

#### Never (Fastest)

```yaml
wal:
  fsync_mode: "never"
```

- OS handles fsync
- Fastest (~44K ops/s)
- Risk of data loss on crash

## Snapshots

### Configuration

```yaml
persistence:
  snapshot:
    enabled: true
    directory: "./data/snapshots"
    interval_secs: 3600  # Every hour
    auto_snapshot: true
    compression: true
    keep_count: 7
```

### Manual Snapshot

```bash
curl -X POST http://localhost:15500/snapshot
```

### List Snapshots

```bash
curl http://localhost:15500/snapshots
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

## Best Practices

### Production Settings

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
    fsync_mode: "periodic"
    fsync_interval_ms: 10
  snapshot:
    enabled: true
    interval_secs: 3600
    keep_count: 7
```

### High Availability

```yaml
persistence:
  wal:
    fsync_mode: "always"  # Maximum safety
  snapshot:
    interval_secs: 900  # Every 15 minutes
    keep_count: 24  # Keep 24 snapshots
```

## Related Topics

- [Persistence Configuration](../configuration/PERSISTENCE.md) - Configuration guide
- [Backup and Restore](../operations/BACKUP.md) - Backup procedures

