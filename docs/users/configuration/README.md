---
title: Configuration
module: configuration
id: configuration-index
order: 0
description: Complete configuration guides
tags: [configuration, settings, server, options]
---

# Configuration

Complete guide to configuring Synap server settings and behavior.

## Configuration Methods

Synap supports multiple configuration methods with the following priority (highest to lowest):

1. **Command line arguments** - Highest priority
2. **Environment variables** - Second priority
3. **YAML configuration file** - Third priority
4. **Default values** - Lowest priority

## Quick Reference

### Essential Configuration

**Minimal configuration (uses defaults):**

```bash
synap-server
```

**Custom host and port:**

```bash
synap-server --host 0.0.0.0 --port 15500
```

**With configuration file:**

```bash
synap-server --config /etc/synap/config.yml
```

## Configuration Guides

- **[Configuration Overview](./CONFIGURATION.md)** - Quick reference and overview
- **[Server Configuration](./SERVER.md)** - Network, ports, host binding
- **[Logging Configuration](./LOGGING.md)** - Log levels, filtering
- **[Persistence Configuration](./PERSISTENCE.md)** - WAL, snapshots, durability
- **[Replication Configuration](./REPLICATION.md)** - Master-replica setup
- **[Performance Tuning](./PERFORMANCE_TUNING.md)** - Optimization tips
- **[Rate Limiting](./RATE_LIMITING.md)** - Rate limiting configuration

## Configuration File Structure

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
    path: "./data/wal/synap.wal"
  snapshot:
    enabled: true
    directory: "./data/snapshots"

replication:
  enabled: false
  role: "master"
```

## Related Topics

- [Installation Guide](../getting-started/INSTALLATION.md) - Installation steps
- [Operations Guide](../operations/SERVICE_MANAGEMENT.md) - Service management

