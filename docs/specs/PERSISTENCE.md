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
Capture the stores under the write barrier, record the WAL cutoff
    │
    ▼
Fsync the WAL up to the cutoff (whatever fsync_mode is)
    │
    ▼
Write snapshot-v5-<sequence>.bin.tmp, fsync, rename, fsync the directory
    │
    ▼
Keep the last K snapshots (never the one just written)
```

The server never truncates the WAL: recovery can always fall back from a
damaged snapshot to an older one (or to the whole WAL) as long as nobody cut
the WAL by hand. See "Snapshot Files and Crash Safety" below.

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

The server does not truncate or rotate the WAL; `WriteAheadLog::truncate`
exists as an API but nothing in the server calls it. A caller that does
must only cut below the cutoff of a snapshot that is already durable (it
was published by `SnapshotManager::write_image`, which fsyncs and renames
before returning) and verified (`SnapshotManager::load` succeeds). After a
cut, recovery refuses to fall back to a snapshot older than the cut (see
"Snapshot Files and Crash Safety"). The sketch below is the historical
design; the current implementation also keeps every entry's original offset.

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
│   ├── snapshot-v5-00000000000000000041.bin
│   ├── snapshot-v5-00000000000000000042.bin
│   └── snapshot-v5-00000000000000000043.bin
│
└── metadata/
    └── recovery.json          # Recovery metadata
```

## Queue Persistence

Queue writes go through `persistence::durable` on every transport (REST,
envelope, RESP3, SynapRPC, MCP) and lock the queue name's ordering stripe like
any key, so they are ordered against snapshots like every other write.

### What is recorded

| Write | Record | Replay |
|---|---|---|
| QCREATE | `QueueCreate { queue, config }` (only when the queue is new) | create with that configuration |
| QPUBLISH | `QueuePublishFull { queue, message }` — every field, `created_at` included | insert the message with its original ID and metadata (`restore_message`) |
| QACK | `QueueAck { queue, message_id }` | remove the message by ID, ready or pending |
| QNACK | `QueueNack { queue, message_id, requeue }` | by ID, ready or pending: retry counter +1, then requeue at the back, dead-letter or drop |
| ACK deadline expiry | `QueueNack { requeue: true }` per expired delivery | as a NACK |
| QPURGE | `QueuePurge { queue, message_ids }` (the ready messages it removed) | remove exactly those IDs |
| QDELETE | `QueueDelete { queue }` (only when the queue existed) | delete the queue |

Deliveries (CONSUME) are not recorded. During a replay, and on a replica, a
delivered message is therefore ready until its ACK or NACK is applied; that is
why ACK, NACK and purge act by message ID in either state. `QueuePublish`
records from older WALs carry no creation time; they are restored with their
ID and every other field.

A replayed ACK, NACK or purge whose message (or queue) is already gone is
expected — a snapshot taken after the message was settled no longer holds it —
and is skipped. Any other replay error fails recovery.

### Queue state in snapshots

Snapshot v5 stores each queue whole: its configuration and its ready,
in-flight and dead-lettered messages with their full metadata (see
`docs/internals/persistence-snapshot-format.md`). On restart, in-flight
messages become ready again for redelivery (no consumer survives a restart),
each ahead of the waiting messages of the same or lower priority, in the order
they were delivered.

### Guarantees after a restart

- An acknowledged message is never deliverable again, whether recovery used
  the WAL alone or a snapshot plus the WAL tail.
- Every unacknowledged message keeps its ID, payload, priority, retry
  counters, creation time and headers.
- A message that was delivered but not acknowledged is delivered again
  (at-least-once). An ACK sent after the restart for such a message returns
  not-found, because the server no longer considers it delivered.
- Queue configuration, purges and deletions survive the restart. Queues
  created before 1.4.0 have no creation record and come back with the
  server's default configuration.
- The redelivery order of messages that were in flight at the restart can
  differ between a WAL-only recovery (they stay where they were published)
  and a snapshot recovery (they go ahead of waiting messages of the same or
  lower priority).

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
1. Read the file header: "SYNAPWAL" + format version (u32 BE). No header means
   a 1.3.x file (format version 1, frames from byte 0); a version newer than
   the server reads fails startup naming both versions
2. Scan the WAL: verify each frame's length (must fit in the file) and CRC32
3. First invalid frame with no valid frame after it (torn tail):
   truncate the file to the last valid frame before anything is appended
4. First invalid frame followed by a valid frame (interior corruption):
   fail startup with the byte position of the bad frame
5. Write the header of a new file, or upgrade a 1.3.x file in place
   (temp file with header + unchanged frames, fsync, rename)
6. Load the newest valid snapshot; a damaged newer one is skipped only when
   the WAL still covers the older snapshot (see "Corrupted Snapshot")
7. Replay WAL from the snapshot offset; a transaction record applies every
   operation it holds
8. New WAL records continue at or above the snapshot's cutoff
9. Resume operations
Time: 1-10 seconds
```

The scanner lives in `persistence/wal_frame.rs` and is shared by the async WAL
and the recovery WAL. A corrupt length that hides the next frame boundary is
handled by searching the rest of the file byte by byte for a valid frame.

### Transaction Records

A committed MULTI/EXEC is logged as ONE `Operation::Transaction { operations }`
record: one frame, one CRC32, one WAL offset. A crash at any byte of it leaves
a torn tail that step 3 discards whole, so recovery holds all of the
transaction's effects or none. Replication ships the same record as one
command at one replication offset; a replica decodes a command only once all
of its bytes arrived, so a dropped connection applies nothing and the resync
delivers the whole record. The variant is the last in `Operation`, so the
bincode variant indices of every older operation, and therefore 1.3.x frames,
are unchanged. Format version 2 files may contain it; 1.3.x binaries cannot
read them (no downgrade).

### Snapshot Consistency

A snapshot is paired exactly with its WAL position (phase23). Every
persisted write, on every transport, goes through `persistence::durable`,
which locks the ordering stripes of the keys it touches (1024 striped
`tokio::sync::Mutex`es, `persistence/order.rs`), applies the write, queues its
record for replication and the WAL, and only then releases the stripes;
durability (group commit, fsync) is awaited afterwards. A snapshot locks
every stripe, places a mark in the WAL writer queue and copies the stores in
memory, releases the stripes, and serializes the copy outside that barrier.
The mark resolves to the offset of the first record queued after it: the
snapshot's cutoff. Every write is therefore either in the image (queued
before the mark, not replayed) or after the cutoff (not in the image,
replayed) — never both, so recovery applies non-idempotent writes such as
LPUSH and HINCRBY exactly once, and multi-key commands and MULTI/EXEC
transactions land wholly on one side.

The same stripes order writes to one key: their records reach the WAL and
the replication stream in the order memory applied them, so replaying
resulting-value records (APPEND, INCR, SETRANGE) ends on the value the key
held. Blocking pops hold the stripes only while they pop, not while they
wait: a blocked client is queued on every key it waits on
(`synap-core` `core/blocking.rs`) and takes the stripes again for each pop
attempt after a push wakes it.

Writers pause while the snapshot copies the stores (measured: ~58 ms for
160,000 KV keys plus 10,000 ten-field hashes in a release build; KV values
are shared buffers, collections are cloned). Serialization and fsync happen
after the barrier is released. With persistence and replication both
disabled no stripe is taken.

### Expiry Across Restarts

Expiry is stored as an absolute Unix-millisecond deadline everywhere it is
persisted:

- Snapshots (v4 and later) store each KV key's deadline (`0` = persistent) and the
  HyperLogLog and bitmap values, which carry `created_at + ttl_secs`.
- WAL records that set or change expiry carry the deadline the store stored:
  `KVSetAt { key, value, expires_at_ms }` (SET EX/PX/EXAT/PXAT/KEEPTTL, and
  INCR/APPEND/SETRANGE on an expiring key), `KVExpireAt { key, expires_at_ms }`
  (EXPIRE) and `HllAddAt { key, elements, expires_at_ms }` (PFADD with a TTL).
  Writes that leave no expiry keep `KVSet { ttl: None }`, `KVExpire { ttl:
  None }` (PERSIST) and `HllAdd { ttl: None }`.
- Records written before 1.4.0 hold TTLs relative to when they were written;
  recovery anchors them at the WAL entry's timestamp (whole seconds), so a
  1.3.2 record setting `k` with a 100 s TTL at time T leaves `k` at most 60 s
  to live when recovered at T + 40 s.

Recovery leaves out every key whose deadline has passed (a write whose
deadline is already past deletes the key), so keys that expired while the
server was down do not come back, and keys that had time left keep exactly
the time they had. Replicas receive the same deadlines, in streamed records
and in the full-sync image (`KVSetAt`).

### Downgrade

1.4.0 reads v2 and v3 snapshots and 1.3.x WALs. 1.3.x cannot read a v4
snapshot (unknown magic) nor a version-2 WAL, so downgrading after 1.4.0 has
written either is not supported; keep a copy of the 1.3.x data directory
before upgrading if a rollback may be needed.

### WAL Write Failures

A failed write, flush or fsync fails every acknowledgement in its group commit
and poisons the async WAL writer: every later append fails until restart, and
`GET /health` answers 503 with `"wal": "poisoned"`. Every EXEC entry point
returns an error when its transaction could not be logged; the in-memory
effects are not rolled back.

### Graceful Shutdown

SIGINT/SIGTERM stop the HTTP listener, wait up to 5 seconds for in-flight
requests, then flush and fsync the WAL and stop its writer, so nothing logged
afterwards is acknowledged. `PersistenceLayer::flush` is an awaited barrier:
it returns once every earlier entry reached the OS (and the disk unless
`fsync_mode` is `never`).

### Snapshot Files and Crash Safety

A snapshot is written to `snapshot-v5-<20-digit sequence>.bin.tmp` in the
snapshot directory (named after its format version), fsynced, renamed to
`snapshot-v5-<sequence>.bin`, and the
directory is fsynced (Unix). On Windows the directory cannot be fsynced; NTFS
renames are atomic and journaled in order, so after a crash the new name
either exists with the complete, already-flushed file or does not exist,
and the retention deletes that follow cannot survive without it. A crash at
any byte of the write therefore leaves the previous snapshots untouched plus,
at most, a temporary file: recovery never lists temporary files and the next
snapshot removes them.

The sequence is one past the highest sequence in the directory and snapshots
run one at a time, so two snapshots in the same second get different names
and a later name always has a later (or equal) WAL cutoff. Recovery orders
files by that sequence, not by the clock. Files named by earlier releases
(`snapshot-v<N>-<unix seconds>.bin`) still load and count as older than every
sequenced file.

Before a snapshot is published the WAL is fsynced up to its cutoff, even with
`fsync_mode: never`, so the WAL never ends below a published snapshot's
cutoff. If it does anyway (a power loss right after a snapshot taken by an
earlier release), recovery logs it and new WAL records continue at the
snapshot's cutoff instead of reusing offsets the next recovery would skip.

### Snapshot Retention

After the new snapshot is durable, the oldest files beyond `max_snapshots` are
removed. At least one snapshot is always kept and the one just written is
never removed (`max_snapshots: 0` keeps one). A failed removal is logged as a
warning; the snapshot itself succeeded.

### Corrupted Snapshot

Recovery tries snapshots newest first. A file that is truncated, fails its
CRC64, has an unknown magic, or holds a record that does not decode is
damaged; every length in the file is checked against the file size before
it is read, so a damaged length cannot trigger a huge allocation.

```
1. Skip a damaged snapshot and try the previous one (or none)
2. Fall back only if the WAL still holds every record after that snapshot's
   cutoff (the whole history when no valid snapshot is left):
   - the WAL is enabled,
   - its first record is at or before the cutoff (offsets start at 1),
   - its offsets have no break after the cutoff,
   - it holds records if that snapshot was taken after some were written
3. Otherwise refuse to start, naming the damaged files and the reason
4. Replay the WAL from the chosen snapshot's cutoff
```

A snapshot that cannot be read for another reason (permissions, device
errors) also refuses startup, since the file may be intact. Before 1.4.0
recovery read only the newest file; when it was damaged the server started
with empty stores, and the next snapshot and retention deleted the older
data. To accept a loss deliberately, restore the damaged file from a backup
or move it out of the snapshot directory before starting the server.

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

Streamed sections ending in a CRC64 digest; the current layout is v5 (magic
`SYNAP005`), described byte by byte in
[`docs/internals/persistence-snapshot-format.md`](../internals/persistence-snapshot-format.md):

```
magic + version + timestamp + wal_offset (the cutoff)
KV (key, value, absolute expiry ms) · queues · streams
hashes · lists · sets · sorted sets · HyperLogLogs · bitmaps
CRC64
```

## Error Handling

```rust
pub enum PersistenceError {
    WALCorrupted { offset: u64, reason: String },
    SnapshotCorrupted { path: PathBuf, reason: String },
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

