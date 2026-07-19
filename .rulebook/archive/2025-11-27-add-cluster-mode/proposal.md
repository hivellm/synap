# Add Cluster Mode

> **Status**: Draft  
> **Priority**: Medium (Phase 4)  
> **Target**: v0.8.0-alpha  
> **Duration**: 12 weeks

## Why

Horizontal write scaling beyond single master. Essential for TB+ datasets and high write throughput.

## What Changes

Implement Redis Cluster with automatic sharding:

**Features**:
- 16,384 hash slots distributed across nodes
- Automatic resharding
- Master-slave replication per shard
- Cluster topology management
- Client-side routing protocol

**Commands**: CLUSTER NODES, CLUSTER INFO, CLUSTER ADDSLOTS, etc.

## Impact

**NEW**: `synap-server/src/cluster/` (~2000 lines)  
**Complexity**: VERY HIGH (Raft/Paxos consensus, slot migration, failover)  
**Risk**: Critical (distributed systems complexity)

