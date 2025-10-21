use super::error::{Result, SynapError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Message ID type
pub type MessageId = String;

/// Consumer ID type
pub type ConsumerId = String;

/// Queue message with metadata
#[derive(Debug, Clone, Serialize)]
pub struct QueueMessage {
    /// Unique message identifier
    pub id: MessageId,
    /// Message payload (bytes)
    pub payload: Vec<u8>,
    /// Priority (0-9, where 9 is highest)
    pub priority: u8,
    /// Number of times this message was retried
    pub retry_count: u32,
    /// Maximum retries allowed
    pub max_retries: u32,
    /// When message was created
    #[serde(skip)]
    pub created_at: Instant,
    /// Custom headers
    pub headers: HashMap<String, String>,
}

impl QueueMessage {
    /// Create a new queue message
    pub fn new(payload: Vec<u8>, priority: u8, max_retries: u32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            payload,
            priority: priority.min(9), // Cap at 9
            retry_count: 0,
            max_retries,
            created_at: Instant::now(),
            headers: HashMap::new(),
        }
    }

    /// Check if message has exceeded retry limit
    pub fn is_dead(&self) -> bool {
        self.retry_count > self.max_retries
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

/// Pending message (delivered but not acknowledged)
#[derive(Debug, Clone)]
struct PendingMessage {
    message: QueueMessage,
    consumer_id: ConsumerId,
    delivered_at: Instant,
    ack_deadline: Instant,
}

/// Queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum queue depth
    pub max_depth: usize,
    /// ACK deadline in seconds
    pub ack_deadline_secs: u64,
    /// Default max retries
    pub default_max_retries: u32,
    /// Default priority
    pub default_priority: u8,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_depth: 100_000,
            ack_deadline_secs: 30,
            default_max_retries: 3,
            default_priority: 5,
        }
    }
}

/// Queue statistics
#[derive(Debug, Default, Clone, Serialize)]
pub struct QueueStats {
    pub depth: usize,
    pub consumers: usize,
    pub published: u64,
    pub consumed: u64,
    pub acked: u64,
    pub nacked: u64,
    pub dead_lettered: u64,
}

/// Single queue instance
#[derive(Debug)]
struct Queue {
    name: String,
    messages: VecDeque<QueueMessage>,
    pending: HashMap<MessageId, PendingMessage>,
    dead_letter: VecDeque<QueueMessage>,
    stats: QueueStats,
    config: QueueConfig,
}

impl Queue {
    fn new(name: String, config: QueueConfig) -> Self {
        Self {
            name,
            messages: VecDeque::new(),
            pending: HashMap::new(),
            dead_letter: VecDeque::new(),
            stats: QueueStats::default(),
            config,
        }
    }

    /// Add message to queue (sorted by priority)
    fn publish(&mut self, message: QueueMessage) -> Result<MessageId> {
        if self.messages.len() >= self.config.max_depth {
            return Err(SynapError::QueueFull(self.name.clone()));
        }

        let message_id = message.id.clone();

        // Insert in priority order (higher priority first)
        let insert_pos = self
            .messages
            .iter()
            .position(|m| m.priority < message.priority)
            .unwrap_or(self.messages.len());

        self.messages.insert(insert_pos, message);
        self.stats.published += 1;
        self.stats.depth = self.messages.len();

        Ok(message_id)
    }

    /// Consume message from queue
    fn consume(&mut self, consumer_id: ConsumerId) -> Option<QueueMessage> {
        if let Some(message) = self.messages.pop_front() {
            let message_id = message.id.clone();
            let ack_deadline = Instant::now() + Duration::from_secs(self.config.ack_deadline_secs);

            // Add to pending
            self.pending.insert(
                message_id,
                PendingMessage {
                    message: message.clone(),
                    consumer_id,
                    delivered_at: Instant::now(),
                    ack_deadline,
                },
            );

            self.stats.consumed += 1;
            self.stats.depth = self.messages.len();
            self.stats.consumers = 1; // Simplified for now

            Some(message)
        } else {
            None
        }
    }

    /// Acknowledge message
    fn ack(&mut self, message_id: &str) -> Result<()> {
        if self.pending.remove(message_id).is_some() {
            self.stats.acked += 1;
            Ok(())
        } else {
            Err(SynapError::MessageNotFound(message_id.to_string()))
        }
    }

    /// Negative acknowledge (requeue or dead letter)
    fn nack(&mut self, message_id: &str, requeue: bool) -> Result<()> {
        if let Some(pending) = self.pending.remove(message_id) {
            let mut message = pending.message;

            self.stats.nacked += 1;

            // Increment retry count first
            message.increment_retry();

            // Then check if exceeded max retries
            if message.is_dead() {
                // Move to dead letter queue
                debug!(
                    "Message {} exceeded retries (retry_count={}, max={}), moving to DLQ",
                    message_id, message.retry_count, message.max_retries
                );
                self.dead_letter.push_back(message);
                self.stats.dead_lettered += 1;
            } else if requeue {
                // Requeue with updated retry count
                debug!(
                    "Requeuing message {} (retry {})",
                    message_id, message.retry_count
                );
                self.messages.push_back(message);
                self.stats.depth = self.messages.len();
            }

            Ok(())
        } else {
            Err(SynapError::MessageNotFound(message_id.to_string()))
        }
    }

    /// Check for expired pending messages
    fn check_expired_pending(&mut self) {
        let now = Instant::now();
        let expired: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, p)| now >= p.ack_deadline)
            .map(|(id, _)| id.clone())
            .collect();

        for message_id in expired {
            debug!("Message {} ACK deadline expired, requeuing", message_id);
            let _ = self.nack(&message_id, true);
        }
    }
}

/// Queue manager (manages multiple queues)
#[derive(Clone)]
pub struct QueueManager {
    queues: Arc<RwLock<HashMap<String, Queue>>>,
    default_config: QueueConfig,
}

impl QueueManager {
    /// Create new queue manager
    pub fn new(config: QueueConfig) -> Self {
        info!("Initializing Queue Manager");
        Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
            default_config: config,
        }
    }

    /// Start background task to check expired pending messages
    pub fn start_deadline_checker(&self) -> tokio::task::JoinHandle<()> {
        let queues = Arc::clone(&self.queues);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));

            loop {
                interval.tick().await;

                let mut queues_guard = queues.write();
                for queue in queues_guard.values_mut() {
                    queue.check_expired_pending();
                }
            }
        })
    }

    /// Create or get queue
    pub async fn create_queue(&self, name: &str, config: Option<QueueConfig>) -> Result<()> {
        debug!("Creating queue: {}", name);

        let mut queues = self.queues.write();
        let queue_config = config.unwrap_or_else(|| self.default_config.clone());

        queues
            .entry(name.to_string())
            .or_insert_with(|| Queue::new(name.to_string(), queue_config));

        Ok(())
    }

    /// Publish message to queue
    pub async fn publish(
        &self,
        queue_name: &str,
        payload: Vec<u8>,
        priority: Option<u8>,
        max_retries: Option<u32>,
    ) -> Result<MessageId> {
        debug!("Publishing to queue: {}", queue_name);

        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;

        let priority = priority.unwrap_or(queue.config.default_priority);
        // If max_retries is not specified (None), use default. If specified (even as 0), use it.
        let max_retries = max_retries.unwrap_or(queue.config.default_max_retries);

        let message = QueueMessage::new(payload, priority, max_retries);
        queue.publish(message)
    }

    /// Consume message from queue
    pub async fn consume(
        &self,
        queue_name: &str,
        consumer_id: &str,
    ) -> Result<Option<QueueMessage>> {
        debug!("Consuming from queue: {}", queue_name);

        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;

        Ok(queue.consume(consumer_id.to_string()))
    }

    /// Acknowledge message
    pub async fn ack(&self, queue_name: &str, message_id: &str) -> Result<()> {
        debug!("ACK message: {} in queue: {}", message_id, queue_name);

        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;

        queue.ack(message_id)
    }

    /// Negative acknowledge message
    pub async fn nack(&self, queue_name: &str, message_id: &str, requeue: bool) -> Result<()> {
        debug!(
            "NACK message: {} in queue: {}, requeue: {}",
            message_id, queue_name, requeue
        );

        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;

        queue.nack(message_id, requeue)
    }

    /// Get queue statistics
    pub async fn stats(&self, queue_name: &str) -> Result<QueueStats> {
        let queues = self.queues.read();
        let queue = queues
            .get(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;

        Ok(queue.stats.clone())
    }

    /// List all queues
    pub async fn list_queues(&self) -> Result<Vec<String>> {
        let queues = self.queues.read();
        Ok(queues.keys().cloned().collect())
    }

    /// Purge queue (remove all messages)
    pub async fn purge(&self, queue_name: &str) -> Result<usize> {
        debug!("Purging queue: {}", queue_name);

        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;

        let count = queue.messages.len();
        queue.messages.clear();
        queue.stats.depth = 0;

        Ok(count)
    }

    /// Delete queue
    pub async fn delete_queue(&self, queue_name: &str) -> Result<bool> {
        debug!("Deleting queue: {}", queue_name);

        let mut queues = self.queues.write();
        Ok(queues.remove(queue_name).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_queue_publish_consume() {
        let manager = QueueManager::new(QueueConfig::default());
        manager.create_queue("test_queue", None).await.unwrap();

        // Publish
        let msg_id = manager
            .publish("test_queue", b"Hello".to_vec(), None, None)
            .await
            .unwrap();

        assert!(!msg_id.is_empty());

        // Consume
        let message = manager.consume("test_queue", "consumer1").await.unwrap();
        assert!(message.is_some());
        assert_eq!(message.unwrap().payload, b"Hello");
    }

    #[tokio::test]
    async fn test_queue_priority() {
        let manager = QueueManager::new(QueueConfig::default());
        manager.create_queue("priority_queue", None).await.unwrap();

        // Publish with different priorities
        manager
            .publish("priority_queue", b"Low".to_vec(), Some(1), None)
            .await
            .unwrap();
        manager
            .publish("priority_queue", b"High".to_vec(), Some(9), None)
            .await
            .unwrap();
        manager
            .publish("priority_queue", b"Medium".to_vec(), Some(5), None)
            .await
            .unwrap();

        // Consume in priority order
        let msg1 = manager
            .consume("priority_queue", "c1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(msg1.payload, b"High");
        assert_eq!(msg1.priority, 9);

        let msg2 = manager
            .consume("priority_queue", "c1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(msg2.payload, b"Medium");
        assert_eq!(msg2.priority, 5);

        let msg3 = manager
            .consume("priority_queue", "c1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(msg3.payload, b"Low");
        assert_eq!(msg3.priority, 1);
    }

    #[tokio::test]
    async fn test_queue_ack_nack() {
        let manager = QueueManager::new(QueueConfig::default());
        manager.create_queue("ack_queue", None).await.unwrap();

        // Publish
        let msg_id = manager
            .publish("ack_queue", b"Test".to_vec(), None, None)
            .await
            .unwrap();

        // Consume
        let message = manager.consume("ack_queue", "c1").await.unwrap().unwrap();
        assert_eq!(message.id, msg_id);

        // ACK
        let result = manager.ack("ack_queue", &msg_id).await;
        assert!(result.is_ok());

        // Second ACK should fail
        let result = manager.ack("ack_queue", &msg_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_queue_nack_requeue() {
        let manager = QueueManager::new(QueueConfig::default());
        manager.create_queue("nack_queue", None).await.unwrap();

        // Publish
        let msg_id = manager
            .publish("nack_queue", b"Test".to_vec(), None, Some(2))
            .await
            .unwrap();

        // Consume
        manager.consume("nack_queue", "c1").await.unwrap();

        // NACK with requeue
        manager.nack("nack_queue", &msg_id, true).await.unwrap();

        // Should be back in queue
        let message = manager.consume("nack_queue", "c1").await.unwrap().unwrap();
        assert_eq!(message.id, msg_id);
        assert_eq!(message.retry_count, 1);
    }

    #[tokio::test]
    async fn test_queue_dead_letter() {
        let manager = QueueManager::new(QueueConfig::default());
        manager.create_queue("dlq_queue", None).await.unwrap();

        // Publish with max 0 retries (first NACK goes to DLQ)
        let msg_id = manager
            .publish("dlq_queue", b"Test".to_vec(), None, Some(0))
            .await
            .unwrap();

        // Consume
        let message = manager.consume("dlq_queue", "c1").await.unwrap();
        assert!(message.is_some());

        // NACK - should go directly to DLQ (retry_count=0 >= max_retries=0)
        manager.nack("dlq_queue", &msg_id, true).await.unwrap();

        // Message should be in DLQ now, queue should be empty
        let stats = manager.stats("dlq_queue").await.unwrap();
        assert_eq!(stats.depth, 0); // No messages in queue
        assert_eq!(stats.dead_lettered, 1); // One in DLQ
        assert_eq!(stats.nacked, 1); // One NACK

        // Verify queue is empty
        let message = manager.consume("dlq_queue", "c1").await.unwrap();
        assert!(message.is_none());
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let manager = QueueManager::new(QueueConfig::default());
        manager.create_queue("stats_queue", None).await.unwrap();

        // Publish 5 messages
        for i in 0..5 {
            manager
                .publish("stats_queue", format!("msg{}", i).into_bytes(), None, None)
                .await
                .unwrap();
        }

        let stats = manager.stats("stats_queue").await.unwrap();
        assert_eq!(stats.depth, 5);
        assert_eq!(stats.published, 5);
    }

    #[tokio::test]
    async fn test_queue_purge() {
        let manager = QueueManager::new(QueueConfig::default());
        manager.create_queue("purge_queue", None).await.unwrap();

        // Add messages
        for i in 0..10 {
            manager
                .publish("purge_queue", format!("msg{}", i).into_bytes(), None, None)
                .await
                .unwrap();
        }

        // Purge
        let count = manager.purge("purge_queue").await.unwrap();
        assert_eq!(count, 10);

        // Verify empty
        let stats = manager.stats("purge_queue").await.unwrap();
        assert_eq!(stats.depth, 0);
    }

    #[tokio::test]
    async fn test_list_queues() {
        let manager = QueueManager::new(QueueConfig::default());

        manager.create_queue("queue1", None).await.unwrap();
        manager.create_queue("queue2", None).await.unwrap();
        manager.create_queue("queue3", None).await.unwrap();

        let queues = manager.list_queues().await.unwrap();
        assert_eq!(queues.len(), 3);
        assert!(queues.contains(&"queue1".to_string()));
        assert!(queues.contains(&"queue2".to_string()));
        assert!(queues.contains(&"queue3".to_string()));
    }

    #[tokio::test]
    async fn test_delete_queue() {
        let manager = QueueManager::new(QueueConfig::default());
        manager.create_queue("temp_queue", None).await.unwrap();

        let deleted = manager.delete_queue("temp_queue").await.unwrap();
        assert!(deleted);

        let deleted = manager.delete_queue("temp_queue").await.unwrap();
        assert!(!deleted);
    }
}
