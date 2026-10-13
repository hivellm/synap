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

### Every transport is logged and replicated

A write is recorded in the WAL, and sent to replicas, whichever transport
carried it: REST, the `/api/v1/command` envelope, RESP3, SynapRPC, MCP (and
UMICP, which goes through MCP) and `redis.call` inside scripts. All of them
call one shared mutation path (`persistence::durable`), so the same command
produces the same log record on every transport. Before 1.4.0 only REST and
the envelope logged, and only for some datatypes: RESP3, SynapRPC, scripts and
the REST/envelope hash, list, set, sorted-set, HyperLogLog, bitmap and
geospatial endpoints were neither durable nor replicated.

Commands whose result depends on the current value are logged as the value
they leave behind, the way transactions are: `INCR`/`DECR` as the resulting
number, `APPEND`/`SETRANGE` as the whole resulting value, `HINCRBY` and
`ZINCRBY` as the resulting field value or score, `SPOP`/`ZPOPMIN`/`ZPOPMAX`
and the blocking pops as the members actually removed. A key's remaining
expiry is logged with the value, so `APPEND` to a key with a TTL keeps the
TTL after a restart. `GEOADD` is logged as the sorted-set scores it wrote.
Writes that change nothing (an `NX` SET on an existing key, an `SREM` of a
missing member) are not logged.

HyperLogLog and bitmap data are rebuilt from the WAL only: the snapshot format
has no section for them yet, so after a snapshot their state before the
snapshot is not restored. Queue publish/ack/nack are logged on every
transport; queue create/delete/purge and streams are not part of the WAL
(streams have their own persistence).

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

- Every group of writes is handed to the OS and fsynced before any of them is
  acknowledged
- Safest, slowest (~1.7K ops/s)
- Zero data loss on crash for acknowledged writes

#### Periodic (Recommended)

```yaml
persistence:
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
    fsync_mode: "periodic"
    fsync_interval_ms: 10
```

- Every group of writes is handed to the OS before it is acknowledged, so a
  process crash loses nothing
- An independent timer fsyncs every `fsync_interval_ms`, whether or not more
  writes arrive, so an idle server still reaches the disk within the interval
- Balanced (~44K ops/s)
- An OS crash or power loss can lose at most the last `fsync_interval_ms` of
  acknowledged writes

#### Never (Fastest)

```yaml
persistence:
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
    fsync_mode: "never"
```

- Every group of writes is handed to the OS before it is acknowledged; Synap
  never calls fsync, and the OS writes pages back on its own schedule
- Fastest (~44K ops/s)
- A process crash loses nothing; an OS crash or power loss can lose any write
  the OS had not yet written back

### Graceful Shutdown

On SIGINT (Ctrl-C) or SIGTERM the server stops accepting HTTP connections,
waits up to 5 seconds for in-flight requests, then flushes and fsyncs the WAL
(in every fsync mode) before exiting. After the drain the WAL refuses every
further append.

### Write Failures and the Poisoned WAL

If a WAL write, flush or fsync fails (disk full, I/O error), every WAL append
in that group fails, and the WAL is **poisoned**: every later WAL append fails
until the server restarts. After a failed fsync the kernel may have
dropped the dirty pages, so the file contents are unknown and the only safe
recovery is a restart that rescans the file.

A poisoned WAL is reported:

- in the log, once, at error level: `WAL writer poisoned: fsync failed: ...;
  every write fails until the server restarts`
- by `GET /health`, which answers **503** with `"status": "unhealthy"`,
  `"wal": "poisoned"` and the reason in `"wal_error"`, so a supervisor's health
  check restarts the server. A healthy server reports `"wal": "ok"` (or
  `"disabled"` without a WAL) with HTTP 200.

A write that cannot be logged fails, in every fsync mode and on every
transport. Writes are applied in memory first and logged second, so the
effect is already in memory and is not rolled back; the error says so:

```text
write applied in memory but could not be logged to the WAL, so it is not durable and is not rolled back: ...
```

REST and the envelope return it as HTTP 500, RESP3 as `-ERR ...`, SynapRPC as
an error response and MCP as a tool error. A transaction (`EXEC`) behaves the
same way with its own wording (see
[Transactions](../../features/transactions.md#when-the-commit-cannot-be-logged)).
Treat either error as "applied in memory, not durable", and monitor `/health`
to catch a poisoned WAL.

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
2. Check the WAL format version, verify every WAL frame (length and CRC32
   checksum) and repair a torn tail
3. Replay WAL from snapshot offset
4. Server ready

### WAL File Format and Versions

Since 1.4.0 a WAL file starts with a 12-byte header: the magic bytes
`SYNAPWAL` followed by the format version (`u32`, big-endian). 1.4.0 writes
format version 2, which adds this header and the single-record MULTI/EXEC
transaction (a committed transaction is one frame under one checksum, so a
crash recovers all of it or none of it; see
[Transactions](../../features/transactions.md#durability--replication-audit-m-010-phase6k)).

- **New WAL**: created with the version 2 header.
- **WAL written by 1.3.x** (no header, frames from byte 0, read as format
  version 1): replayed as before. Before the first append, the server upgrades
  it in place: it writes the header and the unchanged frames to
  `synap.wal.upgrade` next to it, fsyncs that file and renames it over the WAL,
  so a crash during the upgrade leaves either the untouched 1.3.x file or the
  complete upgraded one. The upgrade is logged at `INFO` with the frame count.
- **WAL from a newer format version**: the server **refuses to start** with an
  error naming both versions, for example
  `WAL "./data/wal/synap.wal" uses format version 3, but this server reads WAL
  format versions up to 2`. Run a release that supports that version.
- **Damaged header** (magic present, version 0 or 1): the server refuses to
  start. A header cut short by a crash while the file was being created holds
  no data and is rewritten.

**No downgrade.** A 1.3.x binary cannot read a WAL written or upgraded by
1.4.0: it would misread the header as a frame and replay nothing. Stop the
server and copy the WAL and snapshot directories before upgrading to 1.4.0, and
restore that copy if you need to go back to 1.3.x (writes made under 1.4.0 are
lost in that case).

### Torn Tails and Corruption

Each WAL frame (after the file header) is `length (u64) | CRC32 (u32) |
payload`. On startup the WAL is scanned before anything is appended, and the
first invalid frame (short header, length past the end of the file, checksum
mismatch, or undecodable payload) is classified:

- **Torn tail**: no complete, valid frame follows it. This is what a crash in
  the middle of an append leaves. The file is truncated to the end of the last
  valid frame (logged as a warning with the byte counts) and startup
  continues, so new writes never land after garbage. A MULTI/EXEC transaction
  is one frame, so a torn transaction is discarded whole.
- **Interior corruption**: a valid frame follows it. Truncating would discard
  acknowledged history, so the server **refuses to start** with an error such
  as `WAL "./data/wal/synap.wal" is corrupted at byte 4096: checksum mismatch
  ...; a valid frame follows at byte 4213`. Restore the WAL from a backup, or
  inspect it and remove the damaged range, before restarting.

A declared frame length larger than the rest of the file is rejected before
any memory is allocated for it.

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

### Check WAL Health

```bash
curl -i http://localhost:15500/health
```

HTTP 200 with `"wal": "ok"` while the WAL accepts writes; HTTP 503 with
`"wal": "poisoned"` and `"wal_error"` after a write, flush or fsync failure.

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

