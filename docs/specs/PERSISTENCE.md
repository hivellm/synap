# Persistence Specification

## Overview

Synap provides optional persistence for Key-Value Store and Queue System to ensure durability beyond in-memory storage.

## Persistence Strategy

### Write-Ahead Log (WAL)

All write operations are logged before execution for crash recovery.

```
Write Operation
    │
    ▼
Append to WAL (disk)
    │
    ▼
Execute in Memory
    │
    ▼
Acknowledge to Client
    │
    ▼
Replicate to Slaves (async)
```

### Snapshot System

Periodic snapshots reduce WAL replay time on restart.

```
Every N minutes OR M operations:
    │
    ▼
Create Snapshot (full state dump)
    │
    ▼
Truncate WAL (remove old entries)
    │
    ▼
Keep last K snapshots
```

## Data Structure

### WAL Implementation

```rust
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

pub struct WriteAheadLog {
    file: BufWriter<File>,
    path: PathBuf,
    current_offset: AtomicU64,
    buffer_size: usize,
}

pub struct WALEntry {
    pub offset: u64,
    pub timestamp: u64,
    pub operation: Operation,
    pub checksum: u32,
}

pub enum Operation {
    KVSet { key: String, value: Vec<u8>, ttl: Option<u64> },
    KVDel { keys: Vec<String> },
    QueuePublish { queue: String, message: QueueMessage },
    QueueAck { queue: String, message_id: MessageId },
}
```

### Snapshot Format

```rust
pub struct Snapshot {
    pub version: u32,
    pub timestamp: u64,
    pub wal_offset: u64,
    pub kv_data: HashMap<String, StoredValue>,
    pub queues: HashMap<String, Queue>,
    pub checksum: u64,
}
```

## WAL Operations

### Write to WAL

```rust
impl WriteAheadLog {
    pub async fn append(&mut self, operation: Operation) -> Result<u64> {
        let offset = self.current_offset.fetch_add(1, Ordering::SeqCst);
        
        let entry = WALEntry {
            offset,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            operation,
            checksum: 0,  // Calculate checksum
        };
        
        // Serialize entry
        let data = bincode::serialize(&entry)?;
        let checksum = crc32(&data);
        
        // Write to file
        self.file.write_u64(data.len() as u64).await?;
        self.file.write_u32(checksum).await?;
        self.file.write_all(&data).await?;
        
        // Optional: fsync for durability
        if self.fsync_enabled {
            self.file.flush().await?;
        }
        
        Ok(offset)
    }
}
```

### Read from WAL

```rust
impl WriteAheadLog {
    pub async fn replay(&self, from_offset: u64) -> Result<Vec<WALEntry>> {
        let mut file = File::open(&self.path).await?;
        let mut entries = Vec::new();
        
        loop {
            // Read entry size
            let size = match file.read_u64().await {
                Ok(s) => s,
                Err(_) => break,  // EOF
            };
            
            let checksum_expected = file.read_u32().await?;
            
            // Read entry data
            let mut data = vec![0u8; size as usize];
            file.read_exact(&mut data).await?;
            
            // Verify checksum
            let checksum_actual = crc32(&data);
            if checksum_actual != checksum_expected {
                return Err(PersistenceError::ChecksumMismatch);
            }
            
            // Deserialize
            let entry: WALEntry = bincode::deserialize(&data)?;
            
            if entry.offset >= from_offset {
                entries.push(entry);
            }
        }
        
        Ok(entries)
    }
}
```

## Snapshot Operations

### Create Snapshot

```rust
pub struct SnapshotManager {
    snapshot_dir: PathBuf,
    interval: Duration,
    max_snapshots: usize,
}

impl SnapshotManager {
    pub async fn create_snapshot(
        &self,
        kv_store: &KVStore,
        queue_manager: &QueueManager,
        wal_offset: u64,
    ) -> Result<PathBuf> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let filename = format!("snapshot-{}.bin", timestamp);
        let path = self.snapshot_dir.join(&filename);
        
        // Collect data
        let snapshot = Snapshot {
            version: 1,
            timestamp,
            wal_offset,
            kv_data: kv_store.dump().await?,
            queues: queue_manager.dump().await?,
            checksum: 0,
        };
        
        // Serialize and write
        let data = bincode::serialize(&snapshot)?;
        let checksum = crc64(&data);
        
        let mut file = File::create(&path).await?;
        file.write_u64(checksum).await?;
        file.write_all(&data).await?;
        file.sync_all().await?;
        
        // Cleanup old snapshots
        self.cleanup_old_snapshots().await?;
        
        Ok(path)
    }
    
    async fn cleanup_old_snapshots(&self) -> Result<()> {
        let mut snapshots = self.list_snapshots().await?;
        snapshots.sort_by_key(|s| s.timestamp);
        
        // Keep only last N snapshots
        while snapshots.len() > self.max_snapshots {
            let old = snapshots.remove(0);
            tokio::fs::remove_file(&old.path).await?;
        }
        
        Ok(())
    }
}
```

### Load from Snapshot

```rust
impl SnapshotManager {
    pub async fn load_latest(
        &self
    ) -> Result<Option<(Snapshot, PathBuf)>> {
        let snapshots = self.list_snapshots().await?;
        
        if snapshots.is_empty() {
            return Ok(None);
        }
        
        // Load most recent snapshot
        let latest = &snapshots[snapshots.len() - 1];
        let mut file = File::open(&latest.path).await?;
        
        // Read and verify checksum
        let checksum_expected = file.read_u64().await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        
        let checksum_actual = crc64(&data);
        if checksum_actual != checksum_expected {
            return Err(PersistenceError::CorruptSnapshot);
        }
        
        // Deserialize
        let snapshot: Snapshot = bincode::deserialize(&data)?;
        
        Ok(Some((snapshot, latest.path.clone())))
    }
}
```

## Recovery Process

### Startup Recovery

```rust
pub async fn recover(
    config: &PersistenceConfig
) -> Result<(KVStore, QueueManager, u64)> {
    let snapshot_mgr = SnapshotManager::new(&config.snapshot_dir);
    let wal = WriteAheadLog::open(&config.wal_path).await?;
    
    // 1. Load latest snapshot (if exists)
    let (kv_store, queue_manager, last_offset) = if let Some((snapshot, _)) = 
        snapshot_mgr.load_latest().await? 
    {
        tracing::info!("Loading snapshot from offset {}", snapshot.wal_offset);
        
        let kv = KVStore::from_snapshot(snapshot.kv_data)?;
        let queues = QueueManager::from_snapshot(snapshot.queues)?;
        
        (kv, queues, snapshot.wal_offset)
    } else {
        tracing::info!("No snapshot found, starting fresh");
        (KVStore::new(), QueueManager::new(), 0)
    };
    
    // 2. Replay WAL from snapshot offset
    tracing::info!("Replaying WAL from offset {}...", last_offset);
    let entries = wal.replay(last_offset).await?;
    
    for entry in entries {
        match entry.operation {
            Operation::KVSet { key, value, ttl } => {
                kv_store.set_internal(&key, value, ttl).await?;
            }
            Operation::KVDel { keys } => {
                for key in keys {
                    kv_store.del_internal(&key).await?;
                }
            }
            Operation::QueuePublish { queue, message } => {
                queue_manager.publish_internal(&queue, message).await?;
            }
            Operation::QueueAck { queue, message_id } => {
                queue_manager.ack_internal(&queue, &message_id).await?;
            }
        }
    }
    
    tracing::info!("Recovery complete. Replayed {} operations", entries.len());
    
    Ok((kv_store, queue_manager, last_offset + entries.len() as u64))
}
```

## Background Tasks

### Auto-Snapshot

```rust
pub async fn auto_snapshot_task(
    kv_store: Arc<KVStore>,
    queue_manager: Arc<QueueManager>,
    snapshot_mgr: Arc<SnapshotManager>,
    wal: Arc<RwLock<WriteAheadLog>>,
    interval: Duration,
) {
    let mut timer = tokio::time::interval(interval);
    
    loop {
        timer.tick().await;
        
        let wal_offset = wal.read().current_offset();
        
        match snapshot_mgr.create_snapshot(
            &kv_store,
            &queue_manager,
            wal_offset
        ).await {
            Ok(path) => {
                tracing::info!("Snapshot created: {:?}", path);
                
                // Truncate WAL
                wal.write().truncate(wal_offset).await.ok();
            }
            Err(e) => {
                etracing::info!("Snapshot failed: {}", e);
            }
        }
    }
}
```

### WAL Truncation

```rust
impl WriteAheadLog {
    pub async fn truncate(&mut self, up_to_offset: u64) -> Result<()> {
        // Create new WAL file
        let new_path = self.path.with_extension("wal.new");
        let mut new_file = File::create(&new_path).await?;
        
        // Copy entries after offset
        let entries = self.replay(up_to_offset).await?;
        for entry in entries {
            let data = bincode::serialize(&entry)?;
            new_file.write_all(&data).await?;
        }
        
        new_file.sync_all().await?;
        
        // Atomic rename
        tokio::fs::rename(&new_path, &self.path).await?;
        
        // Reopen file
        self.file = BufWriter::new(File::create(&self.path).await?);
        
        Ok(())
    }
}
```

## Configuration

```yaml
persistence:
  enabled: true
  
  # Write-Ahead Log
  wal:
    enabled: true
    path: "/var/lib/synap/wal/synap.wal"
    buffer_size_kb: 64
    fsync_mode: "periodic"  # "always", "periodic", "never"
    fsync_interval_ms: 1000
    max_size_mb: 1024
  
  # Snapshots
  snapshot:
    enabled: true
    directory: "/var/lib/synap/snapshots"
    interval_secs: 300        # Every 5 minutes
    operation_threshold: 10000 # Or every 10K operations
    max_snapshots: 10          # Keep last 10 snapshots
    compression: true
    compression_level: 6
  
  # Recovery
  recovery:
    verify_checksums: true
    skip_corrupted: false     # Fail on corruption
    repair_mode: false        # Try to repair if true
```

## Durability Modes

### 1. No Persistence (In-Memory Only)

```yaml
persistence:
  enabled: false
```

**Characteristics**:
- ✅ Maximum performance
- ✅ Lowest latency (<1ms)
- ❌ Data lost on crash
- **Use**: Development, caching, ephemeral data

### 2. WAL Only

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
    fsync_mode: "periodic"
  snapshot:
    enabled: false
```

**Characteristics**:
- ✅ Durability with minimal latency
- ✅ Fast recovery (replay WAL)
- ⚠️ Slower recovery as WAL grows
- **Use**: Moderate durability needs

### 3. WAL + Snapshots (Recommended)

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
    fsync_mode: "periodic"
  snapshot:
    enabled: true
    interval_secs: 300
```

**Characteristics**:
- ✅ Fast recovery (snapshot + partial WAL)
- ✅ Bounded recovery time
- ✅ Good balance of performance/durability
- **Use**: Production deployments

### 4. Synchronous Persistence

```yaml
persistence:
  enabled: true
  wal:
    enabled: true
    fsync_mode: "always"  # fsync after every write
```

**Characteristics**:
- ✅ Maximum durability (no data loss)
- ❌ Higher latency (~10ms per write)
- **Use**: Critical data requiring zero loss

## Performance Impact

### Latency Overhead

| Mode | Write Latency | Recovery Time | Use Case |
|------|---------------|---------------|----------|
| No Persistence | 0.5ms | N/A | Dev/Cache |
| WAL (periodic fsync) | 1-2ms | Seconds-Minutes | Production |
| WAL (always fsync) | 10-20ms | Seconds | Critical data |
| WAL + Snapshot | 1-2ms | Seconds | Recommended |

### Throughput Impact

```
No Persistence:    500K ops/sec
WAL (periodic):    200K ops/sec (60% reduction)
WAL (always):      10K ops/sec  (98% reduction)
```

## Storage Layout

```
/var/lib/synap/
├── wal/
│   └── synap.wal              # Write-ahead log
│
├── snapshots/
│   ├── snapshot-1697410800.bin
│   ├── snapshot-1697411100.bin
│   └── snapshot-1697411400.bin
│
└── metadata/
    └── recovery.json          # Recovery metadata
```

## Queue Persistence

### Queue State

Persisted queue state includes:
- Messages in queue (VecDeque)
- Pending messages (HashMap)
- Consumer state
- Dead letter queue

```rust
pub struct QueueSnapshot {
    pub queued: Vec<QueueMessage>,
    pub pending: Vec<PendingMessage>,
    pub dead_letter: Vec<DeadLetteredMessage>,
    pub stats: QueueStats,
}
```

### ACK Durability

```rust
impl Queue {
    pub async fn ack_with_persistence(
        &mut self,
        message_id: &MessageId,
        wal: &mut WriteAheadLog,
    ) -> Result<()> {
        // 1. Write to WAL
        wal.append(Operation::QueueAck {
            queue: self.name.clone(),
            message_id: message_id.clone(),
        }).await?;
        
        // 2. Remove from pending
        self.pending.remove(message_id);
        
        Ok(())
    }
}
```

## Recovery Scenarios

### Normal Restart

```
1. Load latest snapshot
2. Replay WAL from snapshot offset
3. Resume operations
Time: 1-5 seconds
```

### Crash Recovery

```
1. Detect incomplete WAL entry
2. Load latest valid snapshot
3. Replay WAL up to last valid entry
4. Discard incomplete entry
5. Resume operations
Time: 1-10 seconds
```

### Corrupted Snapshot

```
1. Skip corrupted snapshot
2. Try previous snapshot
3. If all snapshots corrupted, replay full WAL
4. Create new snapshot
Time: 10-60 seconds
```

## Replication with Persistence

### Master Node

```
Write → WAL → Memory → Replicate
  ↓
Snapshot (periodic)
```

Master maintains both WAL and snapshots.

### Replica Node

```
Receive Log → Apply to Memory → Optional WAL
  ↓
Optional Snapshot
```

Replicas can optionally maintain their own WAL/snapshots for faster recovery.

## Disaster Recovery

### Backup Strategy

```bash
# Automated backup script
#!/bin/bash
SNAPSHOT_DIR="/var/lib/synap/snapshots"
BACKUP_DIR="/backup/synap"
DATE=$(date +%Y%m%d)

# Copy latest snapshot
cp $SNAPSHOT_DIR/snapshot-*.bin $BACKUP_DIR/snapshot-$DATE.bin

# Compress
gzip $BACKUP_DIR/snapshot-$DATE.bin

# Upload to S3
aws s3 cp $BACKUP_DIR/snapshot-$DATE.bin.gz \
  s3://backups/synap/snapshot-$DATE.bin.gz
```

### Restore from Backup

```bash
# Download backup
aws s3 cp s3://backups/synap/snapshot-20251015.bin.gz .

# Decompress
gunzip snapshot-20251015.bin.gz

# Copy to snapshot directory
cp snapshot-20251015.bin /var/lib/synap/snapshots/

# Restart Synap (will load snapshot)
systemctl restart synap
```

## File Formats

### WAL Binary Format

```
┌────────────────────────────────────┐
│  Entry Size (u64, 8 bytes)         │
├────────────────────────────────────┤
│  Checksum (u32, 4 bytes)           │
├────────────────────────────────────┤
│  Serialized Entry (bincode)        │
│  ├─ offset (u64)                   │
│  ├─ timestamp (u64)                │
│  ├─ operation (enum)               │
│  │   └─ operation-specific data    │
│  └─ checksum (u32)                 │
└────────────────────────────────────┘
```

### Snapshot Binary Format

```
┌────────────────────────────────────┐
│  Checksum (u64, 8 bytes)           │
├────────────────────────────────────┤
│  Compressed Data (zstd)            │
│  └─ Serialized Snapshot (bincode)  │
│      ├─ version (u32)              │
│      ├─ timestamp (u64)            │
│      ├─ wal_offset (u64)           │
│      ├─ kv_data (HashMap)          │
│      ├─ queues (HashMap)           │
│      └─ checksum (u64)             │
└────────────────────────────────────┘
```

## Error Handling

```rust
pub enum PersistenceError {
    WALCorrupted { offset: u64, reason: String },
    SnapshotCorrupted { path: PathBuf },
    ChecksumMismatch { expected: u64, actual: u64 },
    IOError(std::io::Error),
    SerializationError(bincode::Error),
    DiskFull,
}
```

## Monitoring

### Persistence Metrics

```json
{
  "wal": {
    "enabled": true,
    "size_bytes": 104857600,
    "current_offset": 123456,
    "fsync_mode": "periodic",
    "writes_per_sec": 5000,
    "fsync_per_sec": 10
  },
  "snapshot": {
    "enabled": true,
    "count": 10,
    "latest_timestamp": 1697410800,
    "latest_offset": 120000,
    "avg_size_mb": 256,
    "last_duration_secs": 5
  }
}
```

## Testing

### Crash Simulation

```rust
#[tokio::test]
async fn test_crash_recovery() {
    let store = KVStore::new_with_persistence(config).await?;
    
    // Write data
    store.set("key1", b"value1", None).await?;
    store.set("key2", b"value2", None).await?;
    
    // Simulate crash (drop without graceful shutdown)
    drop(store);
    
    // Recover
    let recovered = KVStore::recover(config).await?;
    
    // Verify data
    assert_eq!(recovered.get("key1").await?.unwrap(), b"value1");
    assert_eq!(recovered.get("key2").await?.unwrap(), b"value2");
}
```

## Best Practices

### 1. Choose Appropriate fsync Mode

- **Development**: `never` (fastest)
- **Production (general)**: `periodic` (good balance)
- **Critical data**: `always` (safest)

### 2. Size Snapshots Appropriately

```yaml
snapshot:
  interval_secs: 300           # More frequent for smaller datasets
  operation_threshold: 100000  # Less frequent for write-heavy
```

### 3. Monitor Disk Usage

```bash
# Check WAL size
du -h /var/lib/synap/wal/

# Alert if > 1GB
if [ $(du -m /var/lib/synap/wal/synap.wal | cut -f1) -gt 1024 ]; then
  echo "WAL size exceeds 1GB, consider more frequent snapshots"
fi
```

### 4. Backup Snapshots

```yaml
# Backup cron
0 */6 * * * /usr/local/bin/backup-synap-snapshots.sh
```

## See Also

- [ARCHITECTURE.md](../ARCHITECTURE.md) - System architecture
- [REPLICATION.md](REPLICATION.md) - Replication with persistence
- [CONFIGURATION.md](../CONFIGURATION.md) - Persistence configuration
- [DEPLOYMENT.md](../DEPLOYMENT.md) - Production deployment

