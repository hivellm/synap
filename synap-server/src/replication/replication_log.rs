use super::types::{ReplicationError, ReplicationOperation, ReplicationResult};
use crate::persistence::types::Operation;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

/// Replication Log - In-memory buffer of operations to replicate
///
/// Design inspired by Redis replication backlog:
/// - Fixed-size circular buffer
/// - Offset-based indexing
/// - Support for partial resync
/// - Automatic cleanup of old operations
#[derive(Clone)]
pub struct ReplicationLog {
    /// Operations buffer (circular)
    operations: Arc<RwLock<VecDeque<ReplicationOperation>>>,

    /// Current offset (monotonically increasing)
    current_offset: Arc<AtomicU64>,

    /// Maximum buffer size
    max_size: usize,

    /// Oldest offset in buffer (for partial resync)
    oldest_offset: Arc<AtomicU64>,
}

impl ReplicationLog {
    /// Create a new replication log
    pub fn new(max_size: usize) -> Self {
        info!("Initializing replication log with max size: {}", max_size);

        Self {
            operations: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            current_offset: Arc::new(AtomicU64::new(0)),
            max_size,
            oldest_offset: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Append operation to replication log
    pub fn append(&self, operation: Operation) -> u64 {
        let offset = self.current_offset.fetch_add(1, Ordering::SeqCst);

        let repl_op = ReplicationOperation {
            offset,
            timestamp: Self::current_timestamp(),
            operation,
        };

        let mut ops = self.operations.write();

        // If buffer is full, remove oldest
        if ops.len() >= self.max_size {
            if let Some(removed) = ops.pop_front() {
                self.oldest_offset
                    .store(removed.offset + 1, Ordering::SeqCst);
                debug!("Replication log full, removed offset {}", removed.offset);
            }
        }

        ops.push_back(repl_op);

        offset
    }

    /// Get operations from a specific offset (for partial resync)
    pub fn get_from_offset(
        &self,
        from_offset: u64,
    ) -> ReplicationResult<Vec<ReplicationOperation>> {
        let ops = self.operations.read();
        let oldest = self.oldest_offset.load(Ordering::SeqCst);

        // Check if offset is too old (full resync required)
        if from_offset < oldest {
            return Err(ReplicationError::FullSyncRequired);
        }

        // Filter operations >= from_offset
        let filtered: Vec<ReplicationOperation> = ops
            .iter()
            .filter(|op| op.offset >= from_offset)
            .cloned()
            .collect();

        debug!(
            "Retrieved {} operations from offset {} (oldest: {}, current: {})",
            filtered.len(),
            from_offset,
            oldest,
            self.current_offset()
        );

        Ok(filtered)
    }

    /// Get current offset
    pub fn current_offset(&self) -> u64 {
        self.current_offset.load(Ordering::SeqCst)
    }

    /// Get oldest offset in buffer
    pub fn oldest_offset(&self) -> u64 {
        self.oldest_offset.load(Ordering::SeqCst)
    }

    /// Get buffer size
    pub fn size(&self) -> usize {
        self.operations.read().len()
    }

    /// Clear all operations (used for failover/promotion)
    pub fn clear(&self) {
        let mut ops = self.operations.write();
        ops.clear();
        info!("Replication log cleared");
    }

    /// Get lag between two offsets
    pub fn calculate_lag(&self, replica_offset: u64) -> u64 {
        let current = self.current_offset();
        current.saturating_sub(replica_offset)
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replication_log_append() {
        let log = ReplicationLog::new(100);

        // Append operations
        for i in 0..10 {
            let op = Operation::KVSet {
                key: format!("key_{}", i),
                value: vec![i as u8],
                ttl: None,
            };
            let offset = log.append(op);
            assert_eq!(offset, i);
        }

        assert_eq!(log.current_offset(), 10);
        assert_eq!(log.size(), 10);
    }

    #[test]
    fn test_replication_log_overflow() {
        let log = ReplicationLog::new(5); // Small buffer

        // Append more than max
        for i in 0..10 {
            let op = Operation::KVSet {
                key: format!("key_{}", i),
                value: vec![i as u8],
                ttl: None,
            };
            log.append(op);
        }

        // Should keep only last 5
        assert_eq!(log.size(), 5);
        assert_eq!(log.oldest_offset(), 5);
        assert_eq!(log.current_offset(), 10);
    }

    #[test]
    fn test_get_from_offset() {
        let log = ReplicationLog::new(100);

        // Append 20 operations
        for i in 0..20 {
            let op = Operation::KVSet {
                key: format!("key_{}", i),
                value: vec![i as u8],
                ttl: None,
            };
            log.append(op);
        }

        // Get from offset 10
        let ops = log.get_from_offset(10).unwrap();
        assert_eq!(ops.len(), 10);
        assert_eq!(ops[0].offset, 10);
        assert_eq!(ops[9].offset, 19);
    }

    #[test]
    fn test_full_sync_required() {
        let log = ReplicationLog::new(5);

        // Fill buffer and overflow
        for i in 0..20 {
            let op = Operation::KVSet {
                key: format!("key_{}", i),
                value: vec![i as u8],
                ttl: None,
            };
            log.append(op);
        }

        // Oldest offset should be 15 (20 - 5)
        assert_eq!(log.oldest_offset(), 15);

        // Requesting from offset 10 should require full sync
        let result = log.get_from_offset(10);
        assert!(matches!(result, Err(ReplicationError::FullSyncRequired)));
    }

    #[test]
    fn test_calculate_lag() {
        let log = ReplicationLog::new(100);

        // Append 50 operations
        for i in 0..50 {
            let op = Operation::KVDel {
                keys: vec![format!("key_{}", i)],
            };
            log.append(op);
        }

        // Replica is at offset 30
        let lag = log.calculate_lag(30);
        assert_eq!(lag, 20); // 50 - 30 = 20 operations behind
    }
}
