pub mod layer;
pub mod queue_persistence;
pub mod recovery;
pub mod snapshot;
pub mod stream_persistence;
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
pub mod wal_optimized;

pub use layer::PersistenceLayer;
pub use queue_persistence::QueuePersistence;
pub use recovery::recover;
pub use snapshot::SnapshotManager;
pub use stream_persistence::{StreamEvent, StreamPersistence};
pub use types::{Operation, PersistenceConfig, PersistenceError, Result, Snapshot, WALEntry};
pub use wal::WriteAheadLog;
pub use wal_async::AsyncWAL;
pub use wal_optimized::OptimizedWAL;

#[cfg(test)]
mod tests;
