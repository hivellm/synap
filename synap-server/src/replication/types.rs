use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use thiserror::Error;

/// Node role in replication topology
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    /// Master node - accepts writes, sends to replicas
    Master,
    /// Replica node - read-only, receives from master
    Replica,
    /// Standalone node - no replication
    #[default]
    Standalone,
}

/// Replication command (sent from master to replica)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplicationCommand {
    /// Full sync - initial snapshot transfer
    FullSync { snapshot_data: Vec<u8>, offset: u64 },

    /// Partial sync - incremental updates from offset
    PartialSync {
        from_offset: u64,
        operations: Vec<ReplicationOperation>,
    },

    /// Single operation replication
    Operation(ReplicationOperation),

    /// Heartbeat - keep connection alive, measure lag
    Heartbeat { master_offset: u64, timestamp: u64 },

    /// Acknowledge - replica confirms receipt
    Ack { replica_id: String, offset: u64 },
}

/// Operation to be replicated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationOperation {
    pub offset: u64,
    pub timestamp: u64,
    pub operation: crate::persistence::types::Operation,
}

/// Replication statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplicationStats {
    /// Master offset (latest operation)
    pub master_offset: u64,
    /// Replica offset (last replicated operation)
    pub replica_offset: u64,
    /// Replication lag in operations
    pub lag_operations: u64,
    /// Replication lag in milliseconds
    pub lag_ms: u64,
    /// Total operations replicated
    pub total_replicated: u64,
    /// Total bytes replicated
    pub total_bytes: u64,
    /// Last heartbeat timestamp
    pub last_heartbeat: u64,
    /// Connection status
    pub connected: bool,
}

/// Replica info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaInfo {
    pub id: String,
    pub address: SocketAddr,
    pub role: NodeRole,
    pub offset: u64,
    pub connected_at: u64,
    pub last_sync: u64,
    pub lag_ms: u64,
}

/// Replication error types
#[derive(Debug, Error)]
pub enum ReplicationError {
    #[error("Not a master node")]
    NotMaster,

    #[error("Not a replica node")]
    NotReplica,

    #[error("Replica not found: {0}")]
    ReplicaNotFound(String),

    #[error("Full sync required (offset mismatch)")]
    FullSyncRequired,

    #[error("Replication lag too high: {0}ms")]
    LagTooHigh(u64),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Invalid offset: expected {expected}, got {actual}")]
    InvalidOffset { expected: u64, actual: u64 },
}

impl From<serde_json::Error> for ReplicationError {
    fn from(e: serde_json::Error) -> Self {
        ReplicationError::SerializationError(e.to_string())
    }
}

impl From<bincode::Error> for ReplicationError {
    fn from(e: bincode::Error) -> Self {
        ReplicationError::SerializationError(e.to_string())
    }
}

pub type ReplicationResult<T> = std::result::Result<T, ReplicationError>;
