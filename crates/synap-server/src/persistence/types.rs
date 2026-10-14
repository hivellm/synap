use crate::core::queue::{QueueConfig, QueueMessage, QueueState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Persistence error types
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("WAL corrupted at offset {offset}: {reason}")]
    WALCorrupted { offset: u64, reason: String },

    #[error("Snapshot {path:?} is damaged: {reason}")]
    SnapshotCorrupted { path: PathBuf, reason: String },

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: u64, actual: u64 },

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Disk full")]
    DiskFull,

    #[error("Invalid WAL entry")]
    InvalidEntry,

    #[error("Recovery failed: {0}")]
    RecoveryFailed(String),

    /// An invalid frame is followed by a valid one, so the damage is not a torn
    /// tail left by a crash and truncating would discard acknowledged history.
    #[error(
        "WAL {path:?} is corrupted at byte {position}: {reason}; a valid frame follows at byte {next_valid}, so this is interior corruption, not a torn tail"
    )]
    WALInteriorCorruption {
        path: PathBuf,
        position: u64,
        next_valid: u64,
        reason: String,
    },

    /// A WAL write, flush or fsync failed. After a failed fsync the kernel may
    /// have dropped dirty pages, so the file contents are unknown and every
    /// later append is refused until the process restarts and rescans the file.
    #[error(
        "WAL writer is poisoned ({0}); this write is not durable and every later write fails until the server restarts"
    )]
    WALPoisoned(String),

    /// The WAL header declares a format version newer than this server can
    /// read. The file was written by a newer Synap; replaying it here would
    /// misread records this version does not know.
    #[error(
        "WAL {path:?} uses format version {found}, but this server reads WAL format versions up to {supported}; run a Synap release that supports version {found} or restore a WAL backup"
    )]
    WALUnsupportedVersion {
        path: PathBuf,
        found: u32,
        supported: u32,
    },

    /// The newest snapshot file(s) are damaged or unreadable and no older
    /// starting point is provably complete (the WAL no longer holds every
    /// record after it), so recovering would lose acknowledged writes.
    #[error("cannot recover from the snapshots: {0}")]
    SnapshotUnrecoverable(String),
}

impl PersistenceError {
    /// Whether this error must stop the server from starting.
    ///
    /// Interior corruption, a WAL written by a newer format and a damaged WAL
    /// header all mean the WAL holds acknowledged history this process cannot
    /// replay safely; so does a damaged snapshot recovery cannot safely fall
    /// back from. Starting fresh would hide that history, append after it and
    /// let the next snapshot and retention delete it, so startup must fail
    /// instead.
    pub fn refuses_startup(&self) -> bool {
        matches!(
            self,
            PersistenceError::WALInteriorCorruption { .. }
                | PersistenceError::WALUnsupportedVersion { .. }
                | PersistenceError::WALCorrupted { .. }
                | PersistenceError::SnapshotCorrupted { .. }
                | PersistenceError::SnapshotUnrecoverable(_)
        )
    }
}

impl From<bincode::error::EncodeError> for PersistenceError {
    fn from(e: bincode::error::EncodeError) -> Self {
        PersistenceError::SerializationError(e.to_string())
    }
}

impl From<bincode::error::DecodeError> for PersistenceError {
    fn from(e: bincode::error::DecodeError) -> Self {
        PersistenceError::SerializationError(e.to_string())
    }
}

impl From<crate::core::error::SynapError> for PersistenceError {
    fn from(e: crate::core::error::SynapError) -> Self {
        PersistenceError::RecoveryFailed(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PersistenceError>;

/// WAL entry representing a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WALEntry {
    pub offset: u64,
    pub timestamp: u64,
    pub operation: Operation,
}

/// Operations that can be persisted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// KV Store SET operation
    KVSet {
        key: String,
        value: Vec<u8>,
        ttl: Option<u64>,
    },

    /// KV Store DELETE operation
    KVDel { keys: Vec<String> },

    /// KV Store RENAME operation
    KVRename { source: String, destination: String },

    /// Queue PUBLISH operation
    QueuePublish {
        queue: String,
        message: QueueMessage,
    },

    /// Queue ACK operation
    QueueAck { queue: String, message_id: String },

    /// Queue NACK operation
    QueueNack {
        queue: String,
        message_id: String,
        requeue: bool,
    },

    /// Stream PUBLISH operation
    StreamPublish {
        room: String,
        event_type: String,
        payload: Vec<u8>,
    },

    /// Hash SET operation
    HashSet {
        key: String,
        field: String,
        value: Vec<u8>,
    },

    /// Hash DELETE operation
    HashDel { key: String, fields: Vec<String> },

    /// Hash INCREMENT operation
    HashIncrBy {
        key: String,
        field: String,
        increment: i64,
    },

    /// Hash INCREMENT BY FLOAT operation
    HashIncrByFloat {
        key: String,
        field: String,
        increment: f64,
    },

    /// List PUSH operation (LPUSH or RPUSH)
    ListPush {
        key: String,
        values: Vec<Vec<u8>>,
        left: bool, // true = LPUSH, false = RPUSH
    },

    /// List POP operation (LPOP or RPOP)
    ListPop {
        key: String,
        count: usize,
        left: bool, // true = LPOP, false = RPOP
    },

    /// List SET operation (LSET)
    ListSet {
        key: String,
        index: i64,
        value: Vec<u8>,
    },

    /// List TRIM operation (LTRIM)
    ListTrim { key: String, start: i64, stop: i64 },

    /// List REMOVE operation (LREM)
    ListRem {
        key: String,
        count: i64,
        value: Vec<u8>,
    },

    /// List INSERT operation (LINSERT)
    ListInsert {
        key: String,
        before: bool,
        pivot: Vec<u8>,
        value: Vec<u8>,
    },

    /// List RPOPLPUSH operation
    ListRpoplpush { source: String, destination: String },

    /// Set ADD operation (SADD)
    SetAdd { key: String, members: Vec<Vec<u8>> },

    /// Set REMOVE operation (SREM)
    SetRem { key: String, members: Vec<Vec<u8>> },

    /// Set MOVE operation (SMOVE)
    SetMove {
        source: String,
        destination: String,
        member: Vec<u8>,
    },

    /// Set INTER STORE operation (SINTERSTORE)
    SetInterStore {
        destination: String,
        keys: Vec<String>,
    },

    /// Set UNION STORE operation (SUNIONSTORE)
    SetUnionStore {
        destination: String,
        keys: Vec<String>,
    },

    /// Set DIFF STORE operation (SDIFFSTORE)
    SetDiffStore {
        destination: String,
        keys: Vec<String>,
    },

    /// Sorted Set ADD operation (ZADD)
    ZAdd {
        key: String,
        member: Vec<u8>,
        score: f64,
        nx: bool,
        xx: bool,
        gt: bool,
        lt: bool,
    },

    /// Sorted Set REMOVE operation (ZREM)
    ZRem { key: String, members: Vec<Vec<u8>> },

    /// Sorted Set INCREMENT BY operation (ZINCRBY)
    ZIncrBy {
        key: String,
        member: Vec<u8>,
        increment: f64,
    },

    /// Sorted Set REMOVE RANGE BY RANK operation (ZREMRANGEBYRANK)
    ZRemRangeByRank { key: String, start: i64, stop: i64 },

    /// Sorted Set REMOVE RANGE BY SCORE operation (ZREMRANGEBYSCORE)
    ZRemRangeByScore { key: String, min: f64, max: f64 },

    /// Sorted Set INTER STORE operation (ZINTERSTORE)
    ZInterStore {
        destination: String,
        keys: Vec<String>,
        weights: Option<Vec<f64>>,
        aggregate: String, // "sum", "min", "max"
    },

    /// Sorted Set UNION STORE operation (ZUNIONSTORE)
    ZUnionStore {
        destination: String,
        keys: Vec<String>,
        weights: Option<Vec<f64>>,
        aggregate: String, // "sum", "min", "max"
    },

    /// Sorted Set DIFF STORE operation (ZDIFFSTORE)
    ZDiffStore {
        destination: String,
        keys: Vec<String>,
    },

    /// A committed MULTI/EXEC transaction, stored as ONE record.
    ///
    /// The WAL writes it as a single frame under a single checksum, so a crash
    /// leaves either the whole record or a torn tail that startup discards
    /// whole; replication ships it as one command. Recovery and replicas apply
    /// every operation in it, in order. It never nests another `Transaction`.
    ///
    /// bincode encodes the variant index, so this variant keeps its position
    /// and every later variant is appended after it: earlier encodings (and
    /// 1.3.x WALs) stay readable. Only WAL format version 2 and later contain it.
    Transaction { operations: Vec<Operation> },

    // phase21 variants, appended after `Transaction` so every earlier index
    // (1.3.x and phase22 WALs, replication streams) keeps decoding.
    /// KV EXPIRE (`Some(seconds)`) or PERSIST (`None`) on an existing key.
    KVExpire { key: String, ttl: Option<u64> },

    /// KV FLUSHDB / FLUSHALL: every KV key is removed.
    KVFlush,

    /// COPY a key of any type (KV, hash, list, set, sorted set).
    KeyCopy {
        source: String,
        destination: String,
        replace: bool,
    },

    /// HyperLogLog PFADD.
    HllAdd {
        key: String,
        elements: Vec<Vec<u8>>,
        ttl: Option<u64>,
    },

    /// HyperLogLog PFMERGE.
    HllMerge {
        destination: String,
        sources: Vec<String>,
    },

    /// Bitmap SETBIT.
    BitmapSetBit { key: String, offset: u64, value: u8 },

    /// Bitmap BITOP (`operation` is AND, OR, XOR or NOT).
    BitmapOp {
        operation: String,
        destination: String,
        sources: Vec<String>,
    },

    /// Bitmap BITFIELD, recorded as the resulting value of every field a
    /// SET or INCRBY sub-operation wrote.
    BitmapField {
        key: String,
        writes: Vec<BitfieldWrite>,
    },

    // phase23 variants: absolute expiry. Appended after the phase21 ones so
    // every earlier index keeps decoding. Writes that leave no expiry keep the
    // older records (`KVSet { ttl: None }`, `KVExpire { ttl: None }`,
    // `HllAdd { ttl: None }`), since "no expiry" is already absolute.
    /// KV SET family that left the key expiring at `expires_at_ms` (Unix
    /// milliseconds; `None` = persistent).
    KVSetAt {
        key: String,
        value: Vec<u8>,
        expires_at_ms: Option<u64>,
    },

    /// KV EXPIRE: the existing key now expires at `expires_at_ms` (Unix ms).
    KVExpireAt { key: String, expires_at_ms: u64 },

    /// HyperLogLog PFADD with a TTL: the HLL now expires at `expires_at_ms`
    /// (Unix ms, whole seconds).
    HllAddAt {
        key: String,
        elements: Vec<Vec<u8>>,
        expires_at_ms: u64,
    },

    // phase24 variants: queue identity and lifecycle. Appended after the
    // phase23 ones so every earlier index keeps decoding.
    /// Queue PUBLISH carrying the whole message, creation time included
    /// (`QueuePublish`, written before phase24, lacks it). Recovery and
    /// replicas insert it with its original ID and metadata.
    QueuePublishFull {
        queue: String,
        message: QueueMessageRecord,
    },

    /// Queue creation (QCREATE), with the configuration the queue got.
    /// bincode is positional: a field added to `QueueConfig` changes this
    /// record's encoding, so such a change needs a new variant.
    QueueCreate { queue: String, config: QueueConfig },

    /// Queue deletion (QDELETE) of an existing queue.
    QueueDelete { queue: String },

    /// Queue purge (QPURGE): the IDs of the ready messages it removed.
    /// Deliveries are not logged, so on replay delivered messages are still
    /// ready; replaying the purge by ID removes exactly what it removed.
    QueuePurge {
        queue: String,
        message_ids: Vec<String>,
    },
}

/// Every field of a queue message, as WAL records, replication and v5
/// snapshots store it.
///
/// `QueueMessage`'s own serde form is the API shape and leaves out
/// `created_at`; this one keeps it, so recovery and replicas rebuild the
/// message exactly: ID, payload, priority, retry counters, creation time and
/// headers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueMessageRecord {
    pub id: String,
    pub payload: Vec<u8>,
    pub priority: u8,
    pub retry_count: u32,
    pub max_retries: u32,
    /// Creation time, Unix seconds.
    pub created_at: u32,
    pub headers: HashMap<String, String>,
}

impl From<&QueueMessage> for QueueMessageRecord {
    fn from(message: &QueueMessage) -> Self {
        Self {
            id: message.id.clone(),
            payload: message.payload.as_ref().clone(),
            priority: message.priority,
            retry_count: message.retry_count,
            max_retries: message.max_retries,
            created_at: message.created_at,
            headers: message.headers.clone(),
        }
    }
}

impl From<QueueMessageRecord> for QueueMessage {
    fn from(record: QueueMessageRecord) -> Self {
        Self {
            id: record.id,
            payload: std::sync::Arc::new(record.payload),
            priority: record.priority,
            retry_count: record.retry_count,
            max_retries: record.max_retries,
            created_at: record.created_at,
            headers: record.headers,
        }
    }
}

/// One queue as a snapshot stores it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueImage {
    /// The queue's configuration (v5+). `None` for v2-v4 files, which did
    /// not store it: those queues load with the server's default.
    pub config: Option<QueueConfig>,
    /// Messages waiting for delivery, in delivery order.
    pub ready: Vec<QueueMessageRecord>,
    /// Messages delivered but not acknowledged, in delivery order (v5+).
    pub in_flight: Vec<QueueMessageRecord>,
    /// Dead-lettered messages, oldest first (v5+).
    pub dead_letter: Vec<QueueMessageRecord>,
}

impl From<&QueueState> for QueueImage {
    fn from(state: &QueueState) -> Self {
        let records = |list: &[QueueMessage]| list.iter().map(QueueMessageRecord::from).collect();
        Self {
            config: Some(state.config.clone()),
            ready: records(&state.ready),
            in_flight: records(&state.in_flight),
            dead_letter: records(&state.dead_letter),
        }
    }
}

impl QueueImage {
    /// The queue state this image describes, for
    /// `QueueManager::restore_queue`; `default_config` stands in for a
    /// configuration the image does not hold (v2-v4 snapshots).
    pub fn into_state(self, default_config: QueueConfig) -> QueueState {
        let messages = |list: Vec<QueueMessageRecord>| list.into_iter().map(Into::into).collect();
        QueueState {
            config: self.config.unwrap_or(default_config),
            ready: messages(self.ready),
            in_flight: messages(self.in_flight),
            dead_letter: messages(self.dead_letter),
        }
    }
}

impl Operation {
    /// Rewrite a relative-TTL record into its absolute-expiry form, using the
    /// time it was written (`written_at_secs`, the WAL entry's timestamp in
    /// Unix seconds). Records written before phase23 carried TTLs relative to
    /// that moment; replaying them relative to boot time would restart every
    /// TTL and bring back keys that expired while the server was down.
    /// Records without a relative TTL are returned unchanged; a transaction
    /// record is rewritten operation by operation.
    pub fn with_absolute_expiry(self, written_at_secs: u64) -> Operation {
        let deadline = |ttl: u64| written_at_secs.saturating_add(ttl).saturating_mul(1_000);
        match self {
            Operation::KVSet {
                key,
                value,
                ttl: Some(ttl),
            } => Operation::KVSetAt {
                key,
                value,
                expires_at_ms: Some(deadline(ttl)),
            },
            Operation::KVExpire {
                key,
                ttl: Some(ttl),
            } => Operation::KVExpireAt {
                key,
                expires_at_ms: deadline(ttl),
            },
            Operation::HllAdd {
                key,
                elements,
                ttl: Some(ttl),
            } => Operation::HllAddAt {
                key,
                elements,
                expires_at_ms: deadline(ttl),
            },
            Operation::Transaction { operations } => Operation::Transaction {
                operations: operations
                    .into_iter()
                    .map(|op| op.with_absolute_expiry(written_at_secs))
                    .collect(),
            },
            other => other,
        }
    }
}

/// One field written by a BITFIELD command, as the value it holds afterwards.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BitfieldWrite {
    pub offset: u64,
    pub width: u64,
    pub signed: bool,
    pub value: i64,
}

/// Snapshot containing full system state, as loaded from a snapshot file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub timestamp: u64,
    pub wal_offset: u64,
    pub kv_data: HashMap<String, Vec<u8>>, // Simplified for now
    /// Every queue with its configuration and messages (v5 keeps in-flight
    /// and dead-lettered messages and the full metadata; older files hold
    /// the ready messages only).
    pub queue_data: HashMap<String, QueueImage>,
    #[serde(default)]
    pub stream_data: HashMap<String, Vec<StreamEvent>>, // Room -> Events
    #[serde(default)]
    pub list_data: HashMap<String, crate::core::ListValue>, // Key -> List
    #[serde(default)]
    pub set_data: HashMap<String, crate::core::SetValue>, // Key -> Set
    #[serde(default)]
    pub sorted_set_data: HashMap<String, Vec<(Vec<u8>, f64)>>, // Key -> Vec<(member, score)>
    #[serde(default)]
    pub hash_data: HashMap<String, HashMap<String, Vec<u8>>>, // Key -> field -> value
    /// Absolute expiry (Unix ms) of every expiring key in `kv_data` (v4+;
    /// empty for v2/v3 files, whose keys load without expiry).
    #[serde(default)]
    pub kv_expires_at_ms: HashMap<String, u64>,
    /// HyperLogLog values, each with its own absolute expiry (v4+).
    #[serde(default)]
    pub hyperloglog_data: HashMap<String, crate::core::HyperLogLogValue>,
    /// Bitmap values, each with its own expiry (v4+).
    #[serde(default)]
    pub bitmap_data: HashMap<String, crate::core::BitmapValue>,
}

/// Stream event for snapshot (simplified from stream::StreamEvent)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub id: String,
    pub offset: u64,
    pub event_type: String,
    pub data: Vec<u8>,
    pub timestamp: u64,
}

/// Persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    pub enabled: bool,
    pub wal: WALConfig,
    pub snapshot: SnapshotConfig,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Persistence enabled by default for data safety
            wal: WALConfig::default(),
            snapshot: SnapshotConfig::default(),
        }
    }
}

/// WAL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WALConfig {
    pub enabled: bool,
    pub path: PathBuf,
    pub buffer_size_kb: usize,
    pub fsync_mode: FsyncMode,
    pub fsync_interval_ms: u64,
    pub max_size_mb: usize,
}

impl Default for WALConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: PathBuf::from("./data/wal/synap.wal"),
            buffer_size_kb: 64,
            fsync_mode: FsyncMode::Periodic,
            fsync_interval_ms: 1000,
            max_size_mb: 1024,
        }
    }
}

/// Snapshot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    pub enabled: bool,
    pub directory: PathBuf,
    pub interval_secs: u64,
    pub operation_threshold: usize,
    pub max_snapshots: usize,
    pub compression: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: PathBuf::from("./data/snapshots"),
            interval_secs: 300, // 5 minutes
            operation_threshold: 10_000,
            max_snapshots: 10,
            compression: false, // Disabled for now
        }
    }
}

/// Fsync mode for WAL
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FsyncMode {
    /// Fsync after every write (safest, slowest)
    Always,
    /// Fsync periodically (balanced)
    Periodic,
    /// Never fsync (fastest, least safe)
    Never,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative TTLs are anchored at the record's own timestamp; records
    /// without one, and records already absolute, are left alone.
    #[test]
    fn legacy_relative_ttls_become_absolute_deadlines() {
        let at = |op: Operation| op.with_absolute_expiry(1_000);
        assert!(matches!(
            at(Operation::KVSet {
                key: "k".into(),
                value: vec![],
                ttl: Some(100)
            }),
            Operation::KVSetAt {
                expires_at_ms: Some(1_100_000),
                ..
            }
        ));
        assert!(matches!(
            at(Operation::KVSet {
                key: "k".into(),
                value: vec![],
                ttl: None
            }),
            Operation::KVSet { ttl: None, .. }
        ));
        assert!(matches!(
            at(Operation::KVExpire {
                key: "k".into(),
                ttl: Some(5)
            }),
            Operation::KVExpireAt {
                expires_at_ms: 1_005_000,
                ..
            }
        ));
        assert!(matches!(
            at(Operation::KVExpire {
                key: "k".into(),
                ttl: None
            }),
            Operation::KVExpire { ttl: None, .. }
        ));
        assert!(matches!(
            at(Operation::HllAdd {
                key: "h".into(),
                elements: vec![],
                ttl: Some(1)
            }),
            Operation::HllAddAt {
                expires_at_ms: 1_001_000,
                ..
            }
        ));
        assert!(matches!(
            at(Operation::KVSetAt {
                key: "k".into(),
                value: vec![],
                expires_at_ms: Some(7)
            }),
            Operation::KVSetAt {
                expires_at_ms: Some(7),
                ..
            }
        ));
        match at(Operation::Transaction {
            operations: vec![
                Operation::KVSet {
                    key: "a".into(),
                    value: vec![],
                    ttl: Some(2),
                },
                Operation::KVDel {
                    keys: vec!["b".into()],
                },
            ],
        }) {
            Operation::Transaction { operations } => {
                assert!(matches!(
                    operations[0],
                    Operation::KVSetAt {
                        expires_at_ms: Some(1_002_000),
                        ..
                    }
                ));
                assert!(matches!(operations[1], Operation::KVDel { .. }));
            }
            other => panic!("expected a transaction, got {other:?}"),
        }
    }

    /// The phase23 variants are appended after every earlier one, so the
    /// bincode tags of 1.3.x, phase21 and phase22 records do not move.
    #[test]
    fn phase23_variants_keep_earlier_tags() {
        let tag = |op: &Operation| {
            let bytes = bincode::serde::encode_to_vec(op, bincode::config::legacy()).unwrap();
            u32::from_le_bytes(bytes[..4].try_into().unwrap())
        };
        assert_eq!(
            tag(&Operation::KVSet {
                key: String::new(),
                value: vec![],
                ttl: None
            }),
            0
        );
        assert_eq!(tag(&Operation::Transaction { operations: vec![] }), 32);
        assert_eq!(
            tag(&Operation::BitmapField {
                key: String::new(),
                writes: vec![]
            }),
            40
        );
        assert_eq!(
            tag(&Operation::KVSetAt {
                key: String::new(),
                value: vec![],
                expires_at_ms: None
            }),
            41
        );
        assert_eq!(
            tag(&Operation::KVExpireAt {
                key: String::new(),
                expires_at_ms: 0
            }),
            42
        );
        assert_eq!(
            tag(&Operation::HllAddAt {
                key: String::new(),
                elements: vec![],
                expires_at_ms: 0
            }),
            43
        );
    }

    /// The phase24 queue variants come after every earlier one, so earlier
    /// tags (including the legacy `QueuePublish`, 3) do not move.
    #[test]
    fn phase24_variants_keep_earlier_tags() {
        let tag = |op: &Operation| {
            let bytes = bincode::serde::encode_to_vec(op, bincode::config::legacy()).unwrap();
            u32::from_le_bytes(bytes[..4].try_into().unwrap())
        };
        let message = QueueMessage::new(b"p".to_vec(), 1, 2);
        assert_eq!(
            tag(&Operation::QueuePublish {
                queue: String::new(),
                message: message.clone(),
            }),
            3
        );
        assert_eq!(
            tag(&Operation::QueuePublishFull {
                queue: String::new(),
                message: QueueMessageRecord::from(&message),
            }),
            44
        );
        assert_eq!(
            tag(&Operation::QueueCreate {
                queue: String::new(),
                config: QueueConfig::default(),
            }),
            45
        );
        assert_eq!(
            tag(&Operation::QueueDelete {
                queue: String::new()
            }),
            46
        );
        assert_eq!(
            tag(&Operation::QueuePurge {
                queue: String::new(),
                message_ids: vec![],
            }),
            47
        );
    }

    /// A record keeps every field of the message, creation time included,
    /// through an encode/decode round trip.
    #[test]
    fn queue_message_record_round_trips_every_field() {
        let mut message = QueueMessage::new(b"payload".to_vec(), 7, 4);
        message.retry_count = 2;
        message.created_at = 1_700_000_123;
        message.headers.insert("h".into(), "v".into());
        let record = QueueMessageRecord::from(&message);
        let bytes = bincode::serde::encode_to_vec(&record, bincode::config::legacy()).unwrap();
        let (decoded, _): (QueueMessageRecord, _) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::legacy()).unwrap();
        assert_eq!(decoded, record);
        let restored = QueueMessage::from(decoded);
        assert_eq!(restored.id, message.id);
        assert_eq!(*restored.payload, b"payload".to_vec());
        assert_eq!(restored.priority, 7);
        assert_eq!(restored.retry_count, 2);
        assert_eq!(restored.max_retries, 4);
        assert_eq!(restored.created_at, 1_700_000_123);
        assert_eq!(restored.headers, message.headers);
    }
}
