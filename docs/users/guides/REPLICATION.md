---
title: Replication
module: guides
id: replication-guide
order: 1
description: Master-replica replication setup and management
tags: [guides, replication, master, replica, ha]
---

# Replication

Complete guide to master-replica replication in Synap.

## Overview

Synap supports master-replica replication for:
- **High Availability**: Automatic failover
- **Read Scaling**: Distribute reads across replicas
- **Data Redundancy**: Multiple copies of data
- **Disaster Recovery**: Backup copies

## Architecture

```
┌─────────────┐
│   Master    │ (Writes)
│  :15500     │
└──────┬──────┘
       │ Replication
       │ :15501
       │
   ┌───┴───┬─────────┐
   │       │         │
┌──▼──┐ ┌──▼──┐  ┌──▼──┐
│Rep 1│ │Rep 2│  │Rep 3│ (Reads)
│:15500│ │:15500│  │:15500│
└─────┘ └─────┘ └─────┘
```

## Master Configuration

### Basic Setup

```yaml
server:
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  role: "master"
  replica_listen_address: "0.0.0.0:15501"
  heartbeat_interval_ms: 1000
  max_lag_ms: 10000

persistence:
  enabled: true
  wal:
    enabled: true
  snapshot:
    enabled: true
```

## Replica Configuration

### Basic Setup

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
  snapshot:
    enabled: true
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
curl http://replica3:15500/kv/get/user:1
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

## Failover

### Automatic Failover

Replicas automatically detect master failure and can be promoted.

### Manual Failover

1. Stop master
2. Promote replica to master
3. Update replica configurations
4. Restart services

## Best Practices

### Network Configuration

- Keep master and replicas in same datacenter
- Use low-latency network (< 1ms)
- Monitor network latency

### Persistence

Always enable persistence on both master and replicas.

### Monitoring

Monitor replication lag:
- Alert if lag > 10 seconds
- Monitor connection status
- Track replication throughput

## Related Topics

- [Replication Configuration](../configuration/REPLICATION.md) - Configuration guide
- [Monitoring Guide](../operations/MONITORING.md) - Monitoring and metrics

