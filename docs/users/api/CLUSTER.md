---
title: Cluster API
module: api
id: cluster-api
order: 6
description: Cluster management endpoints
tags: [api, cluster, management, nodes, slots]
---

# Cluster API

Complete guide to cluster management endpoints in Synap.

## Overview

Synap cluster mode provides:
- **Hash Slot Distribution**: CRC16 mod 16384
- **Node Management**: Add, remove, update nodes
- **Slot Migration**: Zero-downtime slot migration
- **Automatic Failover**: Raft consensus for coordination
- **Redis-Compatible**: Compatible with Redis Cluster protocol

## Endpoints

### Cluster Information

**GET** `/cluster/info`

Get cluster information and status.

**Response:**
```json
{
  "cluster_state": "ok",
  "cluster_slots_assigned": 16384,
  "cluster_slots_ok": 16384,
  "cluster_known_nodes": 3,
  "cluster_size": 3,
  "cluster_current_epoch": 1,
  "cluster_my_epoch": 1,
  "cluster_stats": {
    "messages_sent": 1000,
    "messages_received": 1000
  }
}
```

### List Nodes

**GET** `/cluster/nodes`

List all nodes in the cluster.

**Response:**
```json
{
  "nodes": [
    {
      "node_id": "node-1",
      "host": "192.168.1.10",
      "port": 15500,
      "role": "master",
      "slots": [[0, 5460]],
      "state": "connected"
    },
    {
      "node_id": "node-2",
      "host": "192.168.1.11",
      "port": 15500,
      "role": "master",
      "slots": [[5461, 10922]],
      "state": "connected"
    }
  ]
}
```

### Get Node Information

**GET** `/cluster/nodes/{node_id}`

Get information about a specific node.

**Response:**
```json
{
  "node_id": "node-1",
  "host": "192.168.1.10",
  "port": 15500,
  "role": "master",
  "slots": [[0, 5460]],
  "state": "connected",
  "replicas": ["node-3"]
}
```

### Add Node

**POST** `/cluster/nodes`

Add a new node to the cluster.

**Request:**
```json
{
  "node_id": "node-4",
  "host": "192.168.1.14",
  "port": 15500,
  "role": "replica",
  "master_id": "node-1"
}
```

**Response:**
```json
{
  "success": true,
  "node_id": "node-4"
}
```

### Remove Node

**DELETE** `/cluster/nodes/{node_id}`

Remove a node from the cluster.

**Response:**
```json
{
  "success": true,
  "node_id": "node-4"
}
```

### Get Slot Assignments

**GET** `/cluster/slots`

Get slot assignments across all nodes.

**Response:**
```json
{
  "slots": [
    {
      "start": 0,
      "end": 5460,
      "master": "node-1",
      "replicas": ["node-3"]
    },
    {
      "start": 5461,
      "end": 10922,
      "master": "node-2",
      "replicas": []
    }
  ]
}
```

### Assign Slots

**POST** `/cluster/slots/assign`

Assign slots to a node.

**Request:**
```json
{
  "node_id": "node-1",
  "slots": [[0, 5460]]
}
```

**Response:**
```json
{
  "success": true,
  "assigned_slots": 5461
}
```

### Start Migration

**POST** `/cluster/migration/start`

Start migrating slots from one node to another.

**Request:**
```json
{
  "slot": 1000,
  "from_node": "node-1",
  "to_node": "node-2"
}
```

**Response:**
```json
{
  "success": true,
  "migration_id": "mig-123",
  "slot": 1000,
  "status": "in_progress"
}
```

### Complete Migration

**POST** `/cluster/migration/complete`

Complete a slot migration.

**Request:**
```json
{
  "migration_id": "mig-123",
  "slot": 1000
}
```

**Response:**
```json
{
  "success": true,
  "migration_id": "mig-123",
  "keys_migrated": 1000
}
```

### Get Migration Status

**GET** `/cluster/migration/{slot}`

Get status of a slot migration.

**Response:**
```json
{
  "slot": 1000,
  "status": "in_progress",
  "from_node": "node-1",
  "to_node": "node-2",
  "keys_migrated": 500,
  "total_keys": 1000,
  "progress": 0.5
}
```

## Hash Slot Algorithm

### Calculate Hash Slot

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

## Cluster Discovery

### MEET Command

Join a node to the cluster:

```bash
curl -X POST http://localhost:15500/cluster/meet \
  -H "Content-Type: application/json" \
  -d '{
    "host": "192.168.1.11",
    "port": 15500
  }'
```

### PING/PONG

Health check between nodes:

```bash
curl -X POST http://localhost:15500/cluster/ping \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "node-2"
  }'
```

## Error Responses

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

### CLUSTERDOWN Error

Cluster is not available:

```json
{
  "error": {
    "type": "CLUSTERDOWN",
    "message": "Cluster is down"
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

- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [Configuration Guide](../configuration/REPLICATION.md) - Replication setup
- [Guides](../guides/CLUSTER.md) - Cluster setup guide

