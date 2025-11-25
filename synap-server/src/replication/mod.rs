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

#[cfg(test)]
mod tests;
