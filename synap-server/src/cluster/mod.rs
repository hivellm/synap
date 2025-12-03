//! Cluster Mode - Distributed sharding with hash slots
//!
//! Implements Redis-style cluster mode with:
//! - Hash slot algorithm (CRC16 mod 16384)
//! - Cluster topology management
//! - Slot migration with zero downtime
//! - Raft consensus for coordination
//! - Automatic failover

pub mod config;
pub mod discovery;
pub mod failover;
pub mod hash_slot;
pub mod migration;
pub mod raft;
pub mod topology;
pub mod types;

pub use config::ClusterConfig;
pub use discovery::{ClusterDiscovery, start_discovery_server};
pub use failover::ClusterFailover;
pub use hash_slot::{HashSlot, hash_slot};
pub use migration::SlotMigrationManager;
pub use raft::RaftNode;
pub use topology::{ClusterTopology, NodeInfo};
pub use types::{
    ClusterCommand, ClusterError, ClusterNode, ClusterResult, ClusterState, SlotAssignment,
    SlotRange,
};

#[cfg(test)]
mod tests;
