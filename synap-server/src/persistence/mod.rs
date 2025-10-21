/// Persistence module for WAL and Snapshots
/// 
/// Provides durability for KV Store and Queue System through:
/// - Write-Ahead Log (WAL) for crash recovery
/// - Async WAL with group commit optimization (10-100x throughput)
/// - Periodic snapshots for fast recovery
/// - Configurable fsync modes for different durability/performance tradeoffs

pub mod types;
pub mod wal;
pub mod wal_async;
pub mod snapshot;
pub mod recovery;
pub mod layer;

pub use types::{
    Operation, PersistenceConfig, PersistenceError, Result, WALEntry, Snapshot,
};
pub use wal::WriteAheadLog;
pub use wal_async::AsyncWAL;
pub use snapshot::SnapshotManager;
pub use recovery::recover;
pub use layer::PersistenceLayer;

#[cfg(test)]
mod tests;

