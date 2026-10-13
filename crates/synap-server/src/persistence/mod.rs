pub mod apply;
pub mod durable;
pub mod layer;
pub mod order;
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
pub mod wal_frame;
pub mod wal_optimized;

pub use apply::{StoreArcs, StoreRefs};
pub use layer::PersistenceLayer;
pub use queue_persistence::QueuePersistence;
pub use recovery::{RecoveredStores, recover, recover_all};
pub use snapshot::{SnapshotImage, SnapshotManager};
pub use stream_persistence::{StreamEvent, StreamPersistence};
pub use types::{
    BitfieldWrite, Operation, PersistenceConfig, PersistenceError, Result, Snapshot, WALEntry,
};
pub use wal::WriteAheadLog;
pub use wal_async::{AsyncWAL, FileSink, WalSink};
pub use wal_optimized::OptimizedWAL;

#[cfg(test)]
mod tests;
