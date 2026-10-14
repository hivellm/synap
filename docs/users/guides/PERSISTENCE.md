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

- Every group of writes is fsynced before it is acknowledged
- Safest, slowest (~1.7K ops/s)
- Zero data loss on crash for acknowledged writes

#### Periodic (Recommended)

```yaml
wal:
  fsync_mode: "periodic"
  fsync_interval_ms: 10
```

- Every group is handed to the OS before it is acknowledged; a timer fsyncs
  every `fsync_interval_ms`, even while the server is idle
- Balanced (~44K ops/s)
- An OS crash or power loss loses at most the last interval of writes

#### Never (Fastest)

```yaml
wal:
  fsync_mode: "never"
```

- Every group is handed to the OS before it is acknowledged; Synap never
  fsyncs
- Fastest (~44K ops/s)
- An OS crash or power loss can lose writes the OS had not written back

Graceful shutdown (SIGINT/SIGTERM) flushes and fsyncs the WAL in every mode. A
failed WAL write, flush or fsync fails the affected WAL appends and poisons the
WAL until restart; `GET /health` then answers 503. See
[Persistence Configuration](../configuration/PERSISTENCE.md#write-failures-and-the-poisoned-wal).

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
2. Verify every WAL frame and truncate a torn tail left by a crash
3. Replay WAL from snapshot offset
4. Server ready

If a corrupted frame is followed by valid ones, the server refuses to start
and reports the byte position instead of discarding history. It also refuses
to start, leaving the WAL unchanged, when the WAL holds a complete record
whose checksum matches but which this release cannot decode, such as one a
newer release wrote. See
[Torn Tails and Corruption](../configuration/PERSISTENCE.md#torn-tails-and-corruption).

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

