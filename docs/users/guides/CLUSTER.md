---
title: Cluster Mode
module: guides
id: cluster-guide
order: 3
description: Cluster setup and management
tags: [guides, cluster, nodes, slots, migration]
---

# Cluster Mode

Complete guide to cluster setup and management in Synap.

## Overview

Synap cluster mode provides:
- **Hash Slot Distribution**: CRC16 mod 16384
- **Node Management**: Add, remove, update nodes
- **Slot Migration**: Zero-downtime slot migration
- **Automatic Failover**: Raft consensus for coordination
- **Redis-Compatible**: Compatible with Redis Cluster protocol

## Cluster Architecture

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   Node 1    │  │   Node 2    │  │   Node 3    │
│ Slots 0-5460│  │Slots 5461-  │  │Slots 10923- │
│             │  │    10922    │  │    16383    │
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘
       │                │                │
       └────────────────┴────────────────┘
              Cluster Network
```

## Initialization

### Create Cluster

```bash
# Initialize first node
curl -X POST http://localhost:15500/cluster/init \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "node-1",
    "slots": [[0, 5460]]
  }'
```

### Add Nodes

```bash
# Add second node
curl -X POST http://localhost:15500/cluster/nodes \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "node-2",
    "host": "192.168.1.11",
    "port": 15500,
    "role": "master",
    "slots": [[5461, 10922]]
  }'
```

## Hash Slot Algorithm

### Calculate Slot

Keys are assigned to slots using CRC16 mod 16384:

```python
import crc16

def get_slot(key):
    # Extract hash tag if present
    if '{' in key and '}' in key:
        start = key.index('{')
        end = key.index('}')
        key = key[start+1:end]
    
    # Calculate CRC16
    crc = crc16.crc16xmodem(key.encode())
    
    # Mod 16384
    return crc % 16384
```

### Hash Tags

Use hash tags to ensure related keys go to same slot:

```python
# These keys will go to same slot
key1 = "user:{123}:profile"
key2 = "user:{123}:settings"
key3 = "user:{123}:data"
```

## Slot Migration

### Start Migration

```bash
curl -X POST http://localhost:15500/cluster/migration/start \
  -H "Content-Type: application/json" \
  -d '{
    "slot": 1000,
    "from_node": "node-1",
    "to_node": "node-2"
  }'
```

### Monitor Migration

```bash
curl http://localhost:15500/cluster/migration/1000
```

### Complete Migration

```bash
curl -X POST http://localhost:15500/cluster/migration/complete \
  -H "Content-Type: application/json" \
  -d '{
    "migration_id": "mig-123",
    "slot": 1000
  }'
```

## Error Handling

### MOVED Error

Key belongs to different node:

```json
{
  "error": {
    "type": "MOVED",
    "slot": 1000,
    "node": "192.168.1.11:15500"
  }
}
```

### ASK Error

Key is migrating:

```json
{
  "error": {
    "type": "ASK",
    "slot": 1000,
    "node": "192.168.1.12:15500"
  }
}
```

## Best Practices

### Use Hash Tags

Ensure related keys are on same node:

```python
# Good: Use hash tags
key = f"user:{{{user_id}}}:profile"

# Bad: No hash tag
key = f"user:{user_id}:profile"
```

### Monitor Cluster Health

```bash
# Check cluster status
curl http://localhost:15500/cluster/info

# Check node status
curl http://localhost:15500/cluster/nodes
```

### Plan Migrations

- Migrate during low-traffic periods
- Monitor migration progress
- Test failover scenarios

## Related Topics

- [Cluster API](../api/CLUSTER.md) - Cluster API reference
- [Configuration Guide](../configuration/REPLICATION.md) - Replication setup

