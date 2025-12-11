---
title: Replication Configuration
module: configuration
id: replication-configuration
order: 5
description: Master-replica replication setup
tags: [configuration, replication, master, replica, ha]
---

# Replication Configuration

Complete guide to configuring master-replica replication in Synap.

## Overview

Synap supports master-replica replication:
- **Master**: Handles all writes
- **Replicas**: Handle reads, replicate from master
- **High Availability**: Automatic failover
- **Read Scaling**: Distribute reads across replicas

## Master Configuration

### Basic Master Setup

```yaml
server:
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  role: "master"
  replica_listen_address: "0.0.0.0:15501"

persistence:
  enabled: true
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
  snapshot:
    enabled: true
    directory: "./data/snapshots"
```

### Master Settings

```yaml
replication:
  enabled: true
  role: "master"
  replica_listen_address: "0.0.0.0:15501"
  heartbeat_interval_ms: 1000
  max_lag_ms: 10000
```

## Replica Configuration

### Basic Replica Setup

```yaml
server:
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  role: "replica"
  master_address: "master-host:15501"
  auto_reconnect: true
  reconnect_delay_ms: 5000

persistence:
  enabled: true
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
  snapshot:
    enabled: true
    directory: "./data/snapshots"
```

### Replica Settings

```yaml
replication:
  enabled: true
  role: "replica"
  master_address: "master-host:15501"
  auto_reconnect: true
  reconnect_delay_ms: 5000
  read_only: true  # Replicas are read-only
```

## Docker Compose Setup

### Master + Replicas

```yaml
version: '3.8'
services:
  synap-master:
    image: hivellm/synap:latest
    ports:
      - "15500:15500"
      - "15501:15501"
    volumes:
      - master-data:/data
      - ./config-master.yml:/etc/synap/config.yml
    command: ["--config", "/etc/synap/config.yml"]
  
  synap-replica-1:
    image: hivellm/synap:latest
    ports:
      - "15502:15500"
    volumes:
      - replica1-data:/data
      - ./config-replica.yml:/etc/synap/config.yml
    command: ["--config", "/etc/synap/config.yml"]
    depends_on:
      - synap-master
  
  synap-replica-2:
    image: hivellm/synap:latest
    ports:
      - "15503:15500"
    volumes:
      - replica2-data:/data
      - ./config-replica.yml:/etc/synap/config.yml
    command: ["--config", "/etc/synap/config.yml"]
    depends_on:
      - synap-master

volumes:
  master-data:
  replica1-data:
  replica2-data:
```

## Usage Pattern

### Write to Master

```bash
# All writes go to master
curl -X POST http://master-host:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"user:1","value":"John Doe"}'
```

### Read from Replicas

```bash
# Reads can go to any replica (load balancing)
curl http://replica1:15500/kv/get/user:1
curl http://replica2:15500/kv/get/user:1
```

## Monitoring

### Check Replication Status

```bash
# Master status
curl http://master-host:15500/metrics | grep replication

# Replica status
curl http://replica1:15500/metrics | grep replication
```

### Replication Lag

```bash
# Check lag in operations
curl http://replica1:15500/metrics | grep replication_lag_operations

# Check lag in milliseconds
curl http://replica1:15500/metrics | grep replication_lag_ms
```

## Configuration Options

### Heartbeat Interval

```yaml
replication:
  heartbeat_interval_ms: 1000  # Check connection every 1 second
```

### Max Lag Threshold

```yaml
replication:
  max_lag_ms: 10000  # Alert if lag > 10 seconds
```

### Auto Reconnect

```yaml
replication:
  auto_reconnect: true
  reconnect_delay_ms: 5000  # Wait 5 seconds before reconnect
```

## Best Practices

### Network Configuration

- Keep master and replicas in same datacenter
- Use low-latency network (< 1ms)
- Monitor network latency

### Persistence

Always enable persistence on both master and replicas:

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
  snapshot:
    enabled: true
```

### Monitoring

Monitor replication lag:

```bash
# Set up alerts for high lag
# Alert if lag > 10 seconds
```

### Read Scaling

Use multiple replicas for read scaling:

```yaml
# Load balance reads across replicas
# replica1:15500
# replica2:15500
# replica3:15500
```

## Troubleshooting

### High Replication Lag

**Causes:**
- Network latency
- Replica overloaded
- Disk I/O bottleneck

**Solutions:**
- Increase `max_lag_ms` threshold
- Add more replicas
- Use faster network/disk

### Replica Disconnected

**Check:**
```bash
# Check replica status
curl http://replica1:15500/health

# Check master logs
docker logs synap-master
```

**Solutions:**
- Check network connectivity
- Verify master address
- Check firewall rules

## Related Topics

- [Configuration Overview](./CONFIGURATION.md) - General configuration
- [Persistence Configuration](./PERSISTENCE.md) - WAL and snapshots
- [Monitoring Guide](../operations/MONITORING.md) - Monitoring and metrics

