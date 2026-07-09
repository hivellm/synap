/// Replication module - Master-Slave architecture for high availability
///
/// Design inspired by Redis Replication:
/// - 1 Master node (accepts writes)
/// - N Replica nodes (read-only)
/// - Async replication (non-blocking)
/// - Manual failover (promote replica to master)
///
/// Features:
/// - Full sync on replica connect (snapshot + incremental)
/// - Partial resync on reconnect (from last offset)
/// - Lag monitoring and metrics
/// - Configurable replication modes
pub mod config;
pub mod failover;
pub mod master;
pub mod replica;
pub mod replication_log;
pub mod sync;
pub mod types;

pub use config::ReplicationConfig;
pub use failover::FailoverManager;
pub use master::MasterNode;
pub use replica::ReplicaNode;
pub use replication_log::ReplicationLog;
pub use types::{
    NodeRole, ReplicationCommand, ReplicationError, ReplicationResult, ReplicationStats,
};

use std::sync::Arc;

/// A live handle to this node's replication role, stored in `AppState` so INFO
/// and metrics can report real replication status — role, connected replicas,
/// offset, and lag — instead of hardcoded placeholders (phase6j item 1.4).
#[derive(Clone)]
pub enum ReplicationHandle {
    /// This node accepts writes and fans them out to replicas.
    Master(Arc<MasterNode>),
    /// This node is read-only and applies operations streamed from a master.
    Replica(Arc<ReplicaNode>),
}

#[cfg(test)]
mod tests;
