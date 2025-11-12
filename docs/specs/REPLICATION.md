# Replication Specification

## Overview

Synap implements master-slave replication with one write master node and multiple read-only replica nodes for high availability and read scaling.

## Architecture

### Node Roles

```
Master Node (Write)
    │
    ├─→ Accepts all write operations
    ├─→ Maintains replication log
    └─→ Streams log to replicas
    
Replica Nodes (Read-Only)
    │
    ├─→ Receive log stream from master
    ├─→ Apply operations locally
    └─→ Serve read requests
```

### Deployment Topology

```
                  ┌──────────────────┐
                  │  Load Balancer   │
                  │  (Read Traffic)  │
                  └──────────────────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  ▼                  ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   Replica 1  │  │   Replica 2  │  │   Replica 3  │
│   (Read)     │  │   (Read)     │  │   (Read)     │
└──────────────┘  └──────────────┘  └──────────────┘
        ▲                  ▲                  ▲
        │                  │                  │
        └──────────────────┴──────────────────┘
                           │
                   Replication Stream
                           │
                  ┌──────────────────┐
                  │     Master       │
                  │    (Write)       │
                  └──────────────────┘
                           ▲
                           │
                  ┌──────────────────┐
                  │   Write Client   │
                  └──────────────────┘
```

## Replication Log

### Log Structure

```rust
pub struct ReplicationLog {
    entries: VecDeque<LogEntry>,
    current_offset: AtomicU64,
    capacity: usize,
}

pub struct LogEntry {
    pub offset: u64,
    pub timestamp: Instant,
    pub operation: Operation,
    pub checksum: u32,
}

pub enum Operation {
    KVSet { key: String, value: Vec<u8>, ttl: Option<Duration> },
    KVDel { keys: Vec<String> },
    QueuePublish { queue: String, message: QueueMessage },
    QueueAck { queue: String, message_id: MessageId },
    StreamPublish { room: String, event: Event },
    PubSubPublish { topic: String, message: Vec<u8> },
}
```

### Log Retention

**Configuration**:
```yaml
replication:
  log_retention_mode: "hybrid"
  log_retention_hours: 24
  log_max_entries: 1000000
```

**Behavior**:
- Log trimmed when both time and count limits exceeded
- Trimming removes oldest entries
- Ensures replica can sync within retention window

## Replication Protocol

### Connection Handshake

```
Replica                              Master
  │                                    │
  │──── HELLO (replica_id) ───────────→│
  │                                    │
  │←──── HELLO_ACK (current_offset) ──│
  │                                    │
  │──── SYNC (from_offset) ───────────→│
  │                                    │
  │←──── SYNC_START ──────────────────│
  │←──── LogEntry 1 ───────────────────│
  │←──── LogEntry 2 ───────────────────│
  │←──── LogEntry 3 ───────────────────│
  │────→ ACK (offset: 3) ──────────────│
  │                                    │
```

### Message Format

**HELLO**:
```json
{
  "type": "hello",
  "replica_id": "replica-1",
  "version": "0.1.0"
}
```

**HELLO_ACK**:
```json
{
  "type": "hello_ack",
  "master_id": "master-1",
  "current_offset": 12345,
  "log_oldest_offset": 10000
}
```

**SYNC**:
```json
{
  "type": "sync",
  "from_offset": 12300
}
```

**LogEntry**:
```json
{
  "type": "log_entry",
  "offset": 12346,
  "timestamp": 1697410800,
  "operation": {
    "type": "kv_set",
    "key": "user:1",
    "value": "...",
    "ttl": 3600
  },
  "checksum": 0x1a2b3c4d
}
```

## Synchronization Modes

### Full Sync

When replica starts or falls too far behind:

1. Replica requests full state snapshot
2. Master sends current state dump
3. Replica loads state
4. Replica switches to incremental sync

### Incremental Sync

Normal operation mode:

1. Replica tracks last applied offset
2. Master streams new log entries
3. Replica applies operations and ACKs
4. Continuous streaming

## Consistency Model

### Write Behavior
- **Master**: Write returns after local commit
- **Replication**: Asynchronous to replicas
- **Durability**: Depends on replica count (0 = no durability)

### Read Behavior
- **Master**: Strongly consistent (reads own writes)
- **Replica**: Eventually consistent (may lag behind master)
- **Lag**: Typically < 10ms, guaranteed < 100ms

### Consistency Levels

**Read from Master**:
```
Client → Master (read)
Guarantee: Immediate consistency
Use: Critical reads requiring latest data
```

**Read from Replica**:
```
Client → Replica (read)
Guarantee: Eventual consistency (< 10ms typical)
Use: High-throughput reads, analytics
```

## Failover & Promotion

### Manual Promotion

When master fails:

1. **Stop Writes**: Prevent split-brain
2. **Choose Replica**: Select most up-to-date replica
3. **Promote**: Configure chosen replica as new master
4. **Repoint**: Update client connections
5. **Restart Others**: Point remaining replicas to new master

**Promotion Command**:
```bash
synap-admin promote-replica \
  --replica-id replica-1 \
  --force
```

### Promotion Process

```rust
impl Replica {
    pub async fn promote_to_master(&mut self) -> Result<()> {
        // 1. Stop replication
        self.stop_sync().await?;
        
        // 2. Switch mode
        self.mode = NodeMode::Master;
        
        // 3. Initialize replication log
        self.replication_log = ReplicationLog::new();
        
        // 4. Start accepting writes
        self.enable_writes();
        
        // 5. Start replication server
        self.start_replication_server().await?;
        
        Ok(())
    }
}
```

## Replication Lag

### Tracking

```rust
pub struct ReplicationStatus {
    pub replica_id: String,
    pub master_offset: u64,
    pub replica_offset: u64,
    pub lag_entries: u64,
    pub lag_time_ms: u64,
    pub last_sync: Instant,
    pub status: ReplicaStatus,
}

pub enum ReplicaStatus {
    Syncing,
    InSync,
    Lagging,
    Disconnected,
}
```

### Lag Calculation

```rust
impl Master {
    pub fn get_replica_lag(&self, replica_id: &str) -> ReplicationStatus {
        let current_offset = self.replication_log.current_offset();
        let replica_offset = self.replica_offsets.get(replica_id);
        
        ReplicationStatus {
            replica_id: replica_id.to_string(),
            master_offset: current_offset,
            replica_offset,
            lag_entries: current_offset - replica_offset,
            lag_time_ms: calculate_lag_time(replica_offset),
            last_sync: self.last_ack_time(replica_id),
            status: determine_status(lag_entries),
        }
    }
}
```

## Configuration

### Master Configuration

```yaml
server:
  role: "master"
  
replication:
  mode: "master"
  listen_host: "0.0.0.0"
  listen_port: 15501
  log_retention_hours: 24
  log_max_entries: 1000000
  min_replicas_for_write: 0  # Don't wait for replicas
  heartbeat_interval_ms: 1000
  replica_timeout_secs: 30
```

### Replica Configuration

```yaml
server:
  role: "replica"
  read_only: true
  
replication:
  mode: "replica"
  master_host: "master.synap.local"
  master_port: 15501
  sync_interval_ms: 100
  reconnect_interval_secs: 5
  full_sync_threshold: 10000  # Full sync if lagging > 10K entries
```

## Network Protocol

### Transport
- **Primary**: Streaming HTTP (chunked transfer)
- **Alternative**: WebSocket for persistent connection
- **Compression**: Optional gzip for log entries

### Batching

Master batches log entries before sending:

```rust
impl Master {
    async fn stream_to_replica(&self, replica: &mut ReplicaConnection) {
        let mut batch = Vec::new();
        let mut interval = tokio::time::interval(Duration::from_millis(10));
        
        loop {
            interval.tick().await;
            
            // Collect new entries
            let entries = self.replication_log
                .get_since(replica.last_offset);
            
            if !entries.is_empty() {
                batch.extend(entries);
                
                // Send batch
                replica.send_batch(&batch).await?;
                batch.clear();
            }
        }
    }
}
```

## Error Scenarios

### Network Partition

```
Master ←─ X ─→ Replica
```

**Behavior**:
- Master continues accepting writes
- Replica marks status as Disconnected
- Replica attempts reconnection (backoff strategy)
- When reconnected, replica syncs from last offset

### Replica Lag Too Large

If replica falls behind beyond log retention:

```rust
if replica_offset < log_oldest_offset {
    // Full sync required
    send_full_snapshot(replica).await?;
}
```

## Monitoring

### Health Checks

**Master Endpoint**: `GET /health/replication`
```json
{
  "role": "master",
  "current_offset": 12345,
  "replicas": [
    {
      "id": "replica-1",
      "offset": 12344,
      "lag_ms": 5,
      "status": "in_sync"
    },
    {
      "id": "replica-2",
      "offset": 12340,
      "lag_ms": 25,
      "status": "lagging"
    }
  ]
}
```

**Replica Endpoint**: `GET /health/replication`
```json
{
  "role": "replica",
  "master_host": "master.synap.local",
  "master_offset": 12345,
  "local_offset": 12344,
  "lag_entries": 1,
  "lag_ms": 5,
  "status": "in_sync",
  "last_sync": "2025-10-15T19:45:30Z"
}
```

## Performance Tuning

### Batch Size
```yaml
replication:
  batch_size: 100          # Entries per batch
  batch_timeout_ms: 10     # Max wait before sending batch
```

**Trade-offs**:
- Larger batches: Better throughput, higher latency
- Smaller batches: Lower latency, more overhead

### Network Optimization
```yaml
replication:
  compression: true
  compression_threshold: 1024  # Compress if batch > 1KB
  tcp_nodelay: true           # Disable Nagle's algorithm
  keepalive_secs: 60
```

## Disaster Recovery

### Backup Strategy
- **Snapshots**: Periodic full state dumps from master
- **Log Archive**: Archive old log segments
- **Restore**: Load snapshot + replay log

### Split-Brain Prevention
- Only one node can be master at a time
- Replica promotion requires explicit operator action
- No automatic failover in V1

## Testing Requirements

### Unit Tests
- Log entry serialization
- Offset tracking
- Batch assembly
- Checksum validation

### Integration Tests
- Master-replica sync
- Replica catch-up after disconnect
- Multiple replicas simultaneously
- Full sync after lag
- Promotion to master

### Chaos Tests
- Network partition handling
- Replica crash and recovery
- Master crash scenarios
- Slow replica performance

### Performance Tests
- Replication lag (target: < 10ms p95)
- Throughput (target: 10K ops/sec replicated)
- Multiple replicas (3+ replicas)
- Large log replay (1M+ entries)

## Example Scenarios

### Normal Operation

```typescript
// Client writes to master
await masterClient.kv.set('user:1', data);

// Immediately read from master (consistent)
const value1 = await masterClient.kv.get('user:1');

// Read from replica (eventually consistent, ~5ms lag)
const value2 = await replicaClient.kv.get('user:1');
```

### Replica Promotion

```bash
# 1. Verify replica is caught up
synap-admin replication status replica-1

# 2. Stop master (planned maintenance)
synap-admin stop master

# 3. Promote replica
synap-admin promote replica-1

# 4. Update client config to point to new master
# 5. Restart old master as replica
```

### Monitoring Lag

```python
# Python monitoring script
client = SynapClient('http://master:15500')

while True:
    status = client.replication.status()
    
    for replica in status['replicas']:
        if replica['lag_ms'] > 50:
            alert(f"Replica {replica['id']} lagging: {replica['lag_ms']}ms")
    
    time.sleep(10)
```

## Error Handling

```rust
pub enum ReplicationError {
    ConnectionFailed(String),
    SyncFailed(String),
    OffsetNotFound(u64),
    ChecksumMismatch { expected: u32, actual: u32 },
    LogTruncated { requested: u64, oldest: u64 },
    ProtocolError(String),
}
```

## Configuration Reference

### Master Node

```yaml
server:
  role: "master"
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  mode: "master"
  listen_port: 15501
  
  # Log settings
  log_retention_hours: 24
  log_max_entries: 1000000
  log_compression: true
  
  # Replica management
  min_replicas_for_write: 0
  replica_timeout_secs: 30
  heartbeat_interval_secs: 5
  
  # Performance
  batch_size: 100
  batch_timeout_ms: 10
```

### Replica Node

```yaml
server:
  role: "replica"
  host: "0.0.0.0"
  port: 15500
  read_only: true

replication:
  enabled: true
  mode: "replica"
  
  # Master connection
  master_host: "master.synap.local"
  master_port: 15501
  
  # Sync settings
  sync_interval_ms: 100
  reconnect_interval_secs: 5
  reconnect_max_attempts: 0  # Infinite
  
  # Recovery
  full_sync_threshold: 10000
  apply_batch_size: 100
```

## Metrics & Monitoring

### Master Metrics

```json
{
  "role": "master",
  "replication": {
    "enabled": true,
    "current_offset": 123456,
    "log_size": 50000,
    "log_oldest_offset": 73456,
    "replicas": [
      {
        "id": "replica-1",
        "status": "in_sync",
        "offset": 123455,
        "lag_entries": 1,
        "lag_ms": 3,
        "connected_at": "2025-10-15T19:00:00Z",
        "last_ack": "2025-10-15T19:45:32Z"
      }
    ]
  }
}
```

### Replica Metrics

```json
{
  "role": "replica",
  "replication": {
    "enabled": true,
    "master_id": "master-1",
    "master_offset": 123456,
    "local_offset": 123455,
    "lag_entries": 1,
    "lag_ms": 3,
    "status": "in_sync",
    "operations_applied": 123455,
    "errors": 0,
    "last_sync": "2025-10-15T19:45:32Z"
  }
}
```

## Operational Procedures

### Adding New Replica

```bash
# 1. Start replica node with master config
synap-server --config replica-config.yml

# Replica automatically connects and syncs

# 2. Verify replication status
synap-admin replication status

# 3. Add replica to load balancer
```

### Removing Replica

```bash
# 1. Remove from load balancer
# 2. Stop replica
synap-admin stop replica-2

# 3. Master detects disconnect and stops tracking
```

### Planned Maintenance

```bash
# Promote replica-1 to master
synap-admin promote replica-1

# Stop old master
synap-admin stop master

# Restart old master as replica
synap-server --config new-replica-config.yml
```

## Consistency Guarantees

### Read Your Writes

Not guaranteed when reading from replica immediately after write to master.

**Solution**: Read from master for consistency
```typescript
// Write to master
await masterClient.kv.set('session:abc', token);

// Read from master (consistent)
const value = await masterClient.kv.get('session:abc');
```

### Monotonic Reads

Guaranteed within single replica:
- Subsequent reads from same replica never go backwards
- Replica offset only increases

### Eventual Consistency

All replicas will eventually:
- Receive all master writes
- Converge to identical state
- Reflect master state within lag window

## Future Enhancements

### V2 - Synchronous Replication

```yaml
replication:
  mode: "sync"
  min_replicas_ack: 1  # Wait for N replicas before confirming write
  sync_timeout_ms: 100
```

### V3 - Raft Consensus

- Multi-master capability
- Automatic failover
- Strong consistency
- Quorum-based writes

## Testing Scenarios

### Test Cases
1. Basic master-replica sync
2. Replica catch-up after disconnect
3. Multiple replicas simultaneously
4. Network partition recovery
5. Replica promotion
6. Large offset gap (full sync)
7. Checksum mismatch handling
8. Slow replica handling

### Performance Tests
- Replication lag under load
- Multiple replica overhead
- Full sync time (1M entries)
- Sustained write throughput

## See Also

- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [KEY_VALUE_STORE.md](KEY_VALUE_STORE.md) - KV replication behavior
- [QUEUE_SYSTEM.md](QUEUE_SYSTEM.md) - Queue replication
- [DEPLOYMENT.md](../DEPLOYMENT.md) - Production deployment

