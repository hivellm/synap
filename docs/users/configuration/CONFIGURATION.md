---
title: Configuration Overview
module: configuration
id: configuration-overview
order: 1
description: Quick reference and overview of configuration options
tags: [configuration, settings, overview]
---

# Configuration Overview

Quick reference and overview of Synap configuration options.

## Configuration Methods

Synap supports multiple configuration methods with priority (highest to lowest):

1. **Command line arguments** - Highest priority
2. **Environment variables** - Second priority
3. **YAML configuration file** - Third priority
4. **Default values** - Lowest priority

## Quick Start

### Minimal Configuration

```bash
synap-server
```

Uses all defaults:
- Host: `0.0.0.0`
- Port: `15500`
- No persistence
- No replication

### With Configuration File

```bash
synap-server --config config.yml
```

### Command Line Arguments

```bash
synap-server --host 0.0.0.0 --port 15500 --config config.yml
```

## Configuration File Structure

```yaml
server:
  host: "0.0.0.0"
  port: 15500

kv_store:
  max_memory_mb: 4096
  eviction_policy: "lru"  # lru, lfu, none

persistence:
  enabled: true
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
    fsync_mode: "periodic"  # always, periodic, never
    fsync_interval_ms: 10
  snapshot:
    enabled: true
    directory: "./data/snapshots"
    interval_secs: 3600
    auto_snapshot: true

replication:
  enabled: false
  role: "master"  # master, replica
  master_address: ""
  replica_listen_address: "0.0.0.0:15501"
  heartbeat_interval_ms: 1000
  max_lag_ms: 10000

authentication:
  enabled: false
  users: []
  api_keys: []

logging:
  level: "info"  # trace, debug, info, warn, error
  format: "json"  # json, text
```

## Configuration Sections

### Server Configuration

- **host**: Bind address (default: `0.0.0.0`)
- **port**: HTTP port (default: `15500`)

### KV Store Configuration

- **max_memory_mb**: Maximum memory in MB (default: unlimited)
- **eviction_policy**: Eviction policy - `lru`, `lfu`, or `none`

### Persistence Configuration

- **enabled**: Enable persistence (default: `false`)
- **wal**: Write-Ahead Log configuration
- **snapshot**: Snapshot configuration

### Replication Configuration

- **enabled**: Enable replication (default: `false`)
- **role**: `master` or `replica`
- **master_address**: Master address (for replicas), as `host:port` with an IP
  or a DNS name (resolved on every connection attempt); overridden by
  `SYNAP_MASTER_ADDRESS`
- **replica_listen_address**: Replication port (for masters)

### Authentication Configuration

- **enabled**: Enable authentication (default: `false`)
- **users**: List of users
- **api_keys**: List of API keys

### Logging Configuration

- **level**: Log level - `trace`, `debug`, `info`, `warn`, `error`
- **format**: Log format - `json` or `text`

### Rate Limiting Configuration

- **enabled**: Enforce per-client rate limits on the HTTP server (default: `false`)
- **requests_per_second**: Tokens refilled per second in each client's bucket (default: `1000`)
- **burst_size**: Bucket capacity, the most requests a client can send at once (default: `100`)

Clients are keyed by TCP peer address before authentication (so credential
checks are bounded) and by authenticated principal after it. A limited request
answers `429` with `Retry-After`; `/health` and `/metrics` are exempt. See
[Rate Limiting](./RATE_LIMITING.md).

### MCP and UMICP Configuration

- **enable_*_tools**: Which tool families MCP and UMICP expose
- **umicp_max_body_bytes**: Largest `POST /umicp` body the server reads
  (default: `16777216`, 16 MiB). A larger body answers `413 Payload Too Large`:
  a `Content-Length` above the limit is refused before the body is read, and a
  chunked body stops being read once it passes the limit.

## Environment Variables

```bash
export SYNAP_HOST=0.0.0.0
export SYNAP_PORT=15500
export RUST_LOG=info
synap-server
```

## Related Topics

- [Server Configuration](./SERVER.md) - Network and server settings
- [Rate Limiting](./RATE_LIMITING.md) - Per-client request limits
- [Logging Configuration](./LOGGING.md) - Log configuration
- [Persistence Configuration](./PERSISTENCE.md) - WAL and snapshots
- [Replication Configuration](./REPLICATION.md) - Master-replica setup

