use crate::core::queue::QueueMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Persistence error types
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("WAL corrupted at offset {offset}: {reason}")]
    WALCorrupted { offset: u64, reason: String },

    #[error("Snapshot corrupted: {0:?}")]
    SnapshotCorrupted(PathBuf),

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
}

impl From<bincode::Error> for PersistenceError {
    fn from(e: bincode::Error) -> Self {
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
}

/// Snapshot containing full system state
#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub timestamp: u64,
    pub wal_offset: u64,
    pub kv_data: HashMap<String, Vec<u8>>, // Simplified for now
    pub queue_data: HashMap<String, Vec<QueueMessage>>, // Simplified for now
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
