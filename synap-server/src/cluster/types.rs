use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use thiserror::Error;

/// Total number of hash slots (Redis-compatible)
pub const TOTAL_SLOTS: u16 = 16384;

/// Cluster node state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClusterState {
    /// Node is starting up
    Starting,
    /// Node is joining cluster
    Joining,
    /// Node is part of cluster
    Connected,
    /// Node is failing over
    Failover,
    /// Node is migrating slots
    Migrating,
    /// Node is importing slots
    Importing,
    /// Node is offline
    Offline,
}

/// Cluster node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Node ID (unique identifier)
    pub id: String,
    /// Node address (IP:port)
    pub address: SocketAddr,
    /// Node state
    pub state: ClusterState,
    /// Slots assigned to this node (start, end)
    pub slots: Vec<SlotRange>,
    /// Master node ID (if this is a replica)
    pub master_id: Option<String>,
    /// Replica node IDs
    pub replica_ids: Vec<String>,
    /// Last ping timestamp
    pub last_ping: u64,
    /// Node flags (master, replica, etc.)
    pub flags: NodeFlags,
}

/// Node flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NodeFlags {
    pub is_master: bool,
    pub is_replica: bool,
    pub is_myself: bool,
    pub is_fail: bool,
    pub is_handshake: bool,
    pub is_noaddr: bool,
}

/// Slot range (inclusive start, inclusive end)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotRange {
    pub start: u16,
    pub end: u16,
}

impl SlotRange {
    pub fn new(start: u16, end: u16) -> Self {
        assert!(start <= end && end < TOTAL_SLOTS);
        Self { start, end }
    }

    pub fn contains(&self, slot: u16) -> bool {
        slot >= self.start && slot <= self.end
    }

    pub fn count(&self) -> u16 {
        self.end - self.start + 1
    }
}

/// Slot assignment (which node owns which slots)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotAssignment {
    /// Node ID that owns the slot
    pub node_id: String,
    /// Slot number
    pub slot: u16,
    /// Migration state (if migrating)
    pub migrating_to: Option<String>,
    /// Import state (if importing)
    pub importing_from: Option<String>,
}

/// Cluster command (for inter-node communication)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterCommand {
    /// Ping - health check
    Ping { node_id: String, timestamp: u64 },
    /// Pong - ping response
    Pong { node_id: String, timestamp: u64 },
    /// Meet - request to join cluster
    Meet {
        node_id: String,
        address: SocketAddr,
    },
    /// Fail - node failure notification
    Fail { node_id: String, timestamp: u64 },
    /// Update slots - notify slot assignment changes
    UpdateSlots {
        node_id: String,
        slots: Vec<SlotRange>,
    },
    /// Migrate slot - request slot migration
    MigrateSlot {
        slot: u16,
        from_node: String,
        to_node: String,
    },
    /// Slot migrated - confirm slot migration complete
    SlotMigrated {
        slot: u16,
        from_node: String,
        to_node: String,
    },
    /// Ask redirect - redirect client to correct node
    AskRedirect { slot: u16, node_id: String },
    /// Moved redirect - permanent redirect
    MovedRedirect { slot: u16, node_id: String },
}

/// Cluster error types
#[derive(Debug, Error)]
pub enum ClusterError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    #[error("Slot not assigned: {0}")]
    SlotNotAssigned(u16),
    #[error("Slot migration in progress: {0}")]
    SlotMigrating(u16),
    #[error("Cluster not initialized")]
    ClusterNotInitialized,
    #[error("Node already exists: {0}")]
    NodeExists(String),
    #[error("Invalid slot range: {0}-{1}")]
    InvalidSlotRange(u16, u16),
    #[error("Raft consensus error: {0}")]
    RaftError(String),
    #[error("Migration error: {0}")]
    MigrationError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Cluster result type
pub type ClusterResult<T> = Result<T, ClusterError>;
