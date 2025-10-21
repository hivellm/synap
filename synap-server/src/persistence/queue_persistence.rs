use super::types::{Operation, PersistenceError, Result, WALConfig};
use super::wal_optimized::OptimizedWAL;
use crate::core::queue::{QueueManager, QueueMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Queue Persistence Layer (RabbitMQ-style)
///
/// Features:
/// - Durable message storage (survives crashes)
/// - ACK/NACK logging
/// - Message recovery on startup
/// - Dead letter queue persistence
#[derive(Clone)]
pub struct QueuePersistence {
    wal: OptimizedWAL,
    /// Track confirmed messages (ACKed and can be deleted from WAL)
    confirmed: Arc<RwLock<HashMap<String, u64>>>, // message_id -> wal_offset
}

impl QueuePersistence {
    /// Create new queue persistence layer
    pub async fn new(config: WALConfig) -> Result<Self> {
        let wal = OptimizedWAL::open(config).await?;

        Ok(Self {
            wal,
            confirmed: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Log queue publish operation
    pub async fn log_publish(&self, queue: String, message: QueueMessage) -> Result<u64> {
        let offset = self
            .wal
            .append(Operation::QueuePublish { queue, message })
            .await?;

        debug!("Queue publish logged at offset {}", offset);
        Ok(offset)
    }

    /// Log queue ACK operation
    pub async fn log_ack(&self, queue: String, message_id: String) -> Result<u64> {
        let offset = self
            .wal
            .append(Operation::QueueAck {
                queue,
                message_id: message_id.clone(),
            })
            .await?;

        // Mark as confirmed
        self.confirmed.write().await.insert(message_id, offset);

        debug!("Queue ACK logged at offset {}", offset);
        Ok(offset)
    }

    /// Log queue NACK operation
    pub async fn log_nack(&self, queue: String, message_id: String, requeue: bool) -> Result<u64> {
        let offset = self
            .wal
            .append(Operation::QueueNack {
                queue,
                message_id,
                requeue,
            })
            .await?;

        debug!("Queue NACK logged at offset {}", offset);
        Ok(offset)
    }

    /// Recover queue state from WAL
    pub async fn recover(
        &self,
        queue_manager: &QueueManager,
        wal_path: &std::path::PathBuf,
    ) -> Result<u64> {
        info!("Recovering queue state from WAL...");

        let entries = self.wal.read_all(wal_path).await?;
        let mut recovered_count = 0;
        let mut acked_messages = HashMap::new();

        // First pass: collect all ACKed messages
        for entry in &entries {
            if let Operation::QueueAck { message_id, .. } = &entry.operation {
                acked_messages.insert(message_id.clone(), true);
            }
        }

        // Second pass: replay operations
        for entry in entries {
            match entry.operation {
                Operation::QueuePublish { queue, message } => {
                    // Only recover if not ACKed
                    if !acked_messages.contains_key(&message.id) {
                        // Recreate queue if doesn't exist
                        let _ = queue_manager.create_queue(&queue, None).await;

                        // Republish message
                        queue_manager
                            .publish(
                                &queue,
                                message.payload.to_vec(),
                                Some(message.priority),
                                Some(message.max_retries),
                            )
                            .await
                            .ok();

                        recovered_count += 1;
                    }
                }
                Operation::QueueNack {
                    queue,
                    message_id,
                    requeue,
                } => {
                    if requeue {
                        debug!("NACK requeue for message {} in queue {}", message_id, queue);
                        // Message will be redelivered via retry logic
                    }
                }
                _ => {} // Ignore non-queue operations
            }
        }

        info!(
            "Queue recovery complete: {} messages recovered",
            recovered_count
        );
        Ok(recovered_count)
    }

    /// Force flush WAL
    pub async fn flush(&self) -> Result<()> {
        self.wal.flush().await
    }

    /// Get current WAL offset
    pub fn current_offset(&self) -> u64 {
        self.wal.current_offset()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::QueueConfig;

    #[tokio::test]
    async fn test_queue_persistence_recovery() {
        let temp_dir = std::env::temp_dir().join("synap_queue_persist_test");
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let wal_path = temp_dir.join("queue.wal");
        let config = WALConfig {
            enabled: true,
            path: wal_path.clone(),
            buffer_size_kb: 64,
            fsync_mode: super::super::types::FsyncMode::Always,
            fsync_interval_ms: 10,
            max_size_mb: 10,
        };

        // Write some operations
        {
            let queue_persist = QueuePersistence::new(config.clone()).await.unwrap();
            let queue_manager = QueueManager::new(QueueConfig::default());

            queue_manager
                .create_queue("test_queue", None)
                .await
                .unwrap();

            // Publish 5 messages
            for i in 0..5 {
                let msg_id = queue_manager
                    .publish("test_queue", format!("msg_{}", i).into_bytes(), None, None)
                    .await
                    .unwrap();

                // Log to WAL
                let msg = queue_manager
                    .consume("test_queue", "test_consumer")
                    .await
                    .unwrap()
                    .unwrap();
                queue_persist
                    .log_publish("test_queue".to_string(), msg.clone())
                    .await
                    .unwrap();

                // ACK first 3 messages
                if i < 3 {
                    queue_persist
                        .log_ack("test_queue".to_string(), msg.id.clone())
                        .await
                        .unwrap();
                }
            }

            queue_persist.flush().await.unwrap();
        }

        // Recovery
        {
            let queue_persist = QueuePersistence::new(config).await.unwrap();
            let queue_manager = QueueManager::new(QueueConfig::default());

            let recovered = queue_persist
                .recover(&queue_manager, &wal_path)
                .await
                .unwrap();

            // Should recover 2 messages (5 published - 3 ACKed)
            assert_eq!(recovered, 2);
        }

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
