# Cluster Mode with HiveHub.Cloud Integration

This specification describes how Synap's cluster mode works with HiveHub.Cloud multi-tenant SaaS deployments.

## Overview

**Good News**: Cluster mode is **fully compatible** with Hub integration due to the scoped key design!

Key principles:
- **Resource scoping happens before clustering** - all keys are scoped with `user_{user_id}:{resource}` before reaching the cluster layer
- **Cluster operates transparently** - hash slot calculation, routing, and replication work with scoped keys
- **User isolation is maintained** - each user's resources are identified by unique key prefixes
- **Distributed quotas are managed** - cluster-wide quota tracking via master node

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Client Request                              │
│                    (with Hub Access Key)                            │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │  Hub Auth Middleware  │
                    │  (Any cluster node)   │
                    └───────────┬───────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Multi-Tenant        │
                    │   Scoping Layer       │
                    │                       │
                    │   "my-queue"  →       │
                    │   "user_550e:my-queue"│
                    └───────────┬───────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Hash Slot           │
                    │   Calculation         │
                    │   CRC16("user_550e:   │
                    │   my-queue") % 16384  │
                    └───────────┬───────────┘
                                │
           ┌────────────────────┼────────────────────┐
           │                    │                    │
           ▼                    ▼                    ▼
   ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
   │   Node 1     │     │   Node 2     │     │   Node 3     │
   │  Slots 0-5461│     │ Slots 5462-  │     │Slots 10923-  │
   │              │     │  10922       │     │  16383       │
   └──────────────┘     └──────────────┘     └──────────────┘
           │                    │                    │
           └────────────────────┼────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │  Cluster Quota        │
                    │  Manager (Master)     │
                    │                       │
                    │  Tracks quotas for    │
                    │  all users across     │
                    │  all nodes            │
                    └───────────────────────┘
```

## Key Components

### 1. Scoped Key Routing

**Flow**:
1. Client sends request: `QUEUE.PUBLISH my-queue {data}`
2. Hub Auth extracts user: `user_id = 550e8400-e29b-41d4-a716-446655440000`
3. Multi-tenant layer scopes key: `user_550e8400e29b41d4a716446655440000:my-queue`
4. Cluster calculates hash slot: `hash_slot("user_550e...:my-queue") = 12345`
5. Router determines target node: `Node owning slot 12345 = Node 3`
6. Request forwarded to Node 3 (if current node ≠ Node 3)
7. Operation executes on Node 3 with scoped key

**Benefits**:
- **Consistent routing**: All resources for a user route to the same slots
- **User isolation**: Different users have different key prefixes → different hash slots
- **Zero cluster modifications needed**: Cluster doesn't need to know about users

### 2. Hash Slot Algorithm

Synap uses Redis-compatible hash slot algorithm:

```
slot = CRC16(scoped_key) % 16384
```

**Example**:
```rust
// User A's queue
let key = "user_550e8400e29b41d4a716446655440000:orders";
let slot = hash_slot(key); // e.g., slot 7823

// User B's queue (same name!)
let key = "user_661f9511f3ac52e5b827557766551111:orders";
let slot = hash_slot(key); // e.g., slot 14022 (different!)
```

**Result**: Even though both users have a queue named "orders", they map to different slots (likely on different nodes).

### 3. Cluster-Wide Quota Management

In standalone mode, quotas are tracked per-server. In cluster mode, quotas must be tracked **cluster-wide** to prevent users from exceeding limits by spreading requests across nodes.

#### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                       Master Node (node-1)                      │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │         Cluster Quota Manager (Authority)                │  │
│  │                                                          │  │
│  │  - Fetches quotas from HiveHub API                      │  │
│  │  - Aggregates usage deltas from all replica nodes       │  │
│  │  - Updates global quota state                           │  │
│  │  - Syncs to replicas via Raft consensus                 │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                               │
                               │ Quota Sync (30s interval)
                               ▼
      ┌────────────────────────────────────────────────┐
      │                                                │
      ▼                                                ▼
┌─────────────────┐                          ┌─────────────────┐
│  Replica Node 2 │                          │  Replica Node 3 │
│                 │                          │                 │
│  Quota Cache    │                          │  Quota Cache    │
│  (60s TTL)      │                          │  (60s TTL)      │
│                 │                          │                 │
│  Usage Deltas   │                          │  Usage Deltas   │
│  (pending sync) │                          │  (pending sync) │
└─────────────────┘                          └─────────────────┘
```

#### Quota Flow

**1. Operation Request** (any node):
```
1. User makes request to Node 2 (replica)
2. Node 2 checks local quota cache (60s TTL)
   - Cache HIT: Use cached quota, check limits
   - Cache MISS: Query master node for quota
3. If quota allows, operation proceeds
4. Usage delta recorded locally
```

**2. Periodic Sync** (every 30s):
```
1. Replica nodes aggregate local usage deltas:
   - storage_added: +5MB
   - storage_removed: -2MB
   - operations: +150
2. Send deltas to master node
3. Master aggregates all deltas
4. Master updates global quotas
5. Master syncs to HiveHub API
6. Master broadcasts updated quotas to replicas via Raft
```

**3. Cache Invalidation**:
```
- Cache TTL: 60 seconds
- If quota exceeded: Immediate cache invalidation
- If user upgrades plan: Hub API invalidation signal
```

#### Implementation

**Master Node Selection**:
- **Strategy**: Lowest node ID is master
- **Failover**: If master fails, next lowest node ID becomes master
- **Raft**: Uses existing Raft consensus for master election

**Usage Tracking**:
```rust
// On any node (replica or master)
cluster_quota_manager.track_storage_add(user_id, 1024); // +1KB
cluster_quota_manager.track_operation(user_id);         // +1 op

// Every 30 seconds
cluster_quota_manager.sync_deltas_to_master().await;
```

**Quota Checking**:
```rust
// Before operation
let quota = cluster_quota_manager.get_quota(user_id).await?;

if !quota.can_use_storage(1024) {
    return Err("Storage quota exceeded");
}

if !quota.can_perform_operation() {
    return Err("Operation quota exceeded");
}

// Proceed with operation...
```

### 4. User Isolation in Cluster Operations

#### Slot Migration

When slots are migrated between nodes (e.g., during rebalancing or node addition):

**Before Migration**:
```
Node 1: Slots 0-8191
  - user_550e:queue1 (slot 5000)
  - user_661f:queue1 (slot 7000)
  - user_772g:stream1 (slot 3000)

Node 2: Slots 8192-16383
  - user_550e:queue2 (slot 12000)
  - user_883h:kv:key1 (slot 15000)
```

**Migration Decision**: Move slots 4000-6000 from Node 1 to Node 2

**After Migration**:
```
Node 1: Slots 0-3999, 6001-8191
  - user_772g:stream1 (slot 3000)
  - user_661f:queue1 (slot 7000)

Node 2: Slots 4000-6000, 8192-16383
  - user_550e:queue1 (slot 5000)  ← Migrated
  - user_550e:queue2 (slot 12000)
  - user_883h:kv:key1 (slot 15000)
```

**User Isolation Maintained**:
- Only keys in slots 4000-6000 migrated
- User ownership unchanged (keys still scoped with user_id)
- No cross-user data leakage possible

#### Replication

**Master-Replica Replication**:
```
1. Master node receives write: SET user_550e:mykey "value"
2. Master replicates to replicas with full scoped key
3. Replicas store: user_550e:mykey → "value"
4. On replica read: Only returns if user authenticated as 550e
```

**Raft Consensus**:
- All cluster state changes go through Raft
- Scoped keys used in Raft log entries
- User context NOT needed in Raft layer (key scoping sufficient)

## Configuration

### Enable Cluster + Hub Mode

```yaml
# synap.yaml

# Hub integration
hub:
  enabled: true
  service_api_key: "${HIVEHUB_SERVICE_API_KEY}"
  base_url: "https://api.hivehub.cloud"

# Cluster mode
cluster:
  enabled: true
  node_id: "node-1"  # Unique per node
  node_address: "10.0.1.10:15502"
  seed_nodes:
    - "10.0.1.10:15502"  # node-1
    - "10.0.1.11:15502"  # node-2
    - "10.0.1.12:15502"  # node-3
  cluster_port: 15502
  require_full_coverage: true
```

### Cluster Quota Configuration

```yaml
# Cluster quota settings (optional - defaults shown)
hub:
  cluster_quota:
    # Quota cache TTL (per node)
    cache_ttl_seconds: 60

    # Usage sync interval (replica → master)
    sync_interval_seconds: 30

    # Master election strategy
    master_election: "lowest_node_id"  # or "raft_leader"
```

## Deployment

### 3-Node Cluster Example

**Node 1** (Master - lowest node_id):
```bash
export HIVEHUB_SERVICE_API_KEY="sk_service_..."
export SYNAP_NODE_ID="node-1"
export SYNAP_NODE_ADDRESS="10.0.1.10:15502"

./synap-server --config synap.yaml
```

**Node 2** (Replica):
```bash
export HIVEHUB_SERVICE_API_KEY="sk_service_..."
export SYNAP_NODE_ID="node-2"
export SYNAP_NODE_ADDRESS="10.0.1.11:15502"

./synap-server --config synap.yaml
```

**Node 3** (Replica):
```bash
export HIVEHUB_SERVICE_API_KEY="sk_service_..."
export SYNAP_NODE_ID="node-3"
export SYNAP_NODE_ADDRESS="10.0.1.12:15502"

./synap-server --config synap.yaml
```

### Load Balancer

```nginx
upstream synap_cluster {
    least_conn;  # Route to least-loaded node
    server 10.0.1.10:15500;
    server 10.0.1.11:15500;
    server 10.0.1.12:15500;
}

server {
    listen 443 ssl http2;
    server_name api.yourdomain.com;

    location / {
        proxy_pass http://synap_cluster;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;

        # Forward Hub access key
        proxy_set_header Authorization $http_authorization;
        proxy_set_header X-Hub-Access-Key $http_x_hub_access_key;
    }
}
```

## Behavior Summary

| Aspect | Standalone Mode | Cluster Mode | Cluster + Hub Mode |
|--------|----------------|--------------|-------------------|
| **Key Scoping** | Optional (Hub only) | N/A (no scoping) | Required (automatic) |
| **Routing** | Direct (local) | Hash slot based | Hash slot + scoped keys |
| **Quota Tracking** | Per-server | Per-server | **Cluster-wide via master** |
| **User Isolation** | Via scoping | N/A | Via scoping (cluster-transparent) |
| **Failover** | Single point | Automatic (Raft) | Automatic (Raft) |
| **Load Distribution** | N/A | Hash slot based | Hash slot + user-aware |

## Advantages

### 1. **Automatic User-Aware Routing**

Users' resources are routed to nodes based on scoped key hashing. This provides:
- **Locality**: All resources for a user may land on same node (if hash aligns)
- **Distribution**: Different users automatically distribute across nodes
- **No hot spots**: User activity spread naturally via hash function

### 2. **Transparent Cluster Operations**

Cluster doesn't need to know about users:
- Slot migration works with scoped keys
- Replication works with scoped keys
- Failover works with scoped keys
- **Simplicity**: No cluster modifications needed for multi-tenancy!

### 3. **Scalable Quota Management**

Master-based quota tracking scales efficiently:
- **Local caching** (60s) reduces master load
- **Batched sync** (30s) reduces network overhead
- **Eventual consistency** acceptable for quotas (soft limits)
- **Raft replication** ensures fault tolerance

### 4. **Strong Isolation Guarantees**

User isolation maintained at all levels:
- Keys are pre-scoped before clustering
- Hash slots don't mix users (probabilistically distributed)
- Migration moves complete scoped keys (no splitting)
- Replication preserves scoped keys

## Limitations

### 1. **Eventual Consistency for Quotas**

- **Issue**: Replicas cache quotas (60s TTL)
- **Risk**: User might slightly exceed quota if requests spread across nodes before sync
- **Mitigation**:
  - Short cache TTL (60s)
  - Frequent sync (30s)
  - Quota soft limits (Hub API enforces hard limits)

### 2. **Master Node Bottleneck**

- **Issue**: All quota updates flow through master
- **Risk**: Master becomes bottleneck at extreme scale
- **Mitigation**:
  - Local caching reduces master queries
  - Batched deltas reduce master load
  - Raft ensures master failover

### 3. **Cross-Node Operations**

- **Issue**: If user's keys spread across multiple slots → multiple nodes
- **Example**: `SET user_550e:key1` on Node 1, `SET user_550e:key2` on Node 2
- **Impact**: Transactions, multi-key operations require cluster coordination
- **Mitigation**: Use hash tags for related keys: `user_550e:{app1}:key1`, `user_550e:{app1}:key2` (same slot)

## Testing

### Verify Multi-Tenant Isolation in Cluster

```bash
# Setup: 3-node cluster running

# User A access key
export USER_A_KEY="sk_live_a1b2c3d4..."

# User B access key
export USER_B_KEY="sk_live_x9y8z7w6..."

# User A: Create queue on any node
curl -X POST http://node-1:15500/queue/publish \
  -H "Authorization: Bearer $USER_A_KEY" \
  -d '{"queue": "orders", "message": "Order #1"}'

# User B: Try to access User A's queue (should fail - not visible)
curl -X GET http://node-2:15500/queue/list \
  -H "Authorization: Bearer $USER_B_KEY"
# Expected: []  (empty - User B sees no queues)

# User A: List queues from different node (should see queue)
curl -X GET http://node-3:15500/queue/list \
  -H "Authorization: Bearer $USER_A_KEY"
# Expected: ["user_<userA_id>:orders"]  (User A sees their queue)
```

### Verify Cluster-Wide Quotas

```bash
# User with 100MB storage quota
export USER_KEY="sk_live_..."

# Add data on Node 1 (50MB)
curl -X POST http://node-1:15500/kv/set \
  -H "Authorization: Bearer $USER_KEY" \
  -d '{"key": "large1", "value": "<50MB data>"}'

# Add data on Node 2 (50MB)
curl -X POST http://node-2:15500/kv/set \
  -H "Authorization: Bearer $USER_KEY" \
  -d '{"key": "large2", "value": "<50MB data>"}'

# Try to add more data (should fail - quota exceeded)
curl -X POST http://node-3:15500/kv/set \
  -H "Authorization: Bearer $USER_KEY" \
  -d '{"key": "large3", "value": "<10MB data>"}'
# Expected: 429 Too Many Requests (storage quota exceeded)
```

## Monitoring

### Cluster Quota Metrics

```
synap_cluster_quota_cache_hits_total
synap_cluster_quota_cache_misses_total
synap_cluster_quota_sync_duration_seconds
synap_cluster_quota_master_queries_total
synap_cluster_quota_exceeded_total{user_id, resource_type}
```

### Per-Node Metrics

```
synap_cluster_node_keys_total{node_id}
synap_cluster_node_requests_total{node_id, user_id}
synap_cluster_node_quota_checks_total{node_id, result}
```

## Future Enhancements

### 1. **Distributed Quota Ledger**

Instead of master-based quotas, use distributed ledger (Raft-based):
- No single master bottleneck
- Stronger consistency guarantees
- More complex implementation

### 2. **User-Aware Slot Assignment**

Instead of random hash slots, assign user's keys to specific nodes:
- All keys for a user on one node (better for transactions)
- Requires custom slot assignment algorithm
- Harder to balance load

### 3. **Quota Overage Allowance**

Allow temporary quota overages:
- Soft limit: 100MB (cached)
- Hard limit: 110MB (API enforced)
- Grace period: 1 hour
- Better UX, prevents abrupt failures

## Summary

**Hub integration is fully compatible with cluster mode** thanks to scoped key design:

✅ **User isolation maintained** - scoped keys prevent cross-user access
✅ **Routing works automatically** - hash slots calculated from scoped keys
✅ **Cluster-wide quotas supported** - master node aggregates usage
✅ **Zero cluster modifications** - cluster layer is user-agnostic
✅ **Scalable architecture** - local caching + periodic sync

The only significant implementation is cluster-wide quota management, which follows a master-replica pattern with Raft-based failover.
