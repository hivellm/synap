use super::error::{Result, SynapError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};
use uuid::Uuid;

/// Message ID type
pub type MessageId = String;

/// Consumer ID type
pub type ConsumerId = String;

/// Queue message with metadata
/// Uses Arc for payload sharing to reduce memory usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMessage {
    /// Unique message identifier
    pub id: MessageId,
    /// Message payload (bytes) - Arc-shared to avoid cloning
    #[serde(
        serialize_with = "serialize_arc_payload",
        deserialize_with = "deserialize_arc_payload"
    )]
    pub payload: Arc<Vec<u8>>,
    /// Priority (0-9, where 9 is highest)
    pub priority: u8,
    /// Number of times this message was retried
    pub retry_count: u32,
    /// Maximum retries allowed
    pub max_retries: u32,
    /// When message was created (Unix timestamp)
    #[serde(skip, default = "current_timestamp")]
    pub created_at: u32,
    /// Custom headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

// Serialization helpers for Arc<Vec<u8>>
fn serialize_arc_payload<S>(
    payload: &Arc<Vec<u8>>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bytes(payload.as_ref())
}

fn deserialize_arc_payload<'de, D>(deserializer: D) -> std::result::Result<Arc<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let vec = Vec::<u8>::deserialize(deserializer)?;
    Ok(Arc::new(vec))
}

fn current_timestamp() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

impl QueueMessage {
    /// Create a new queue message
    pub fn new(payload: Vec<u8>, priority: u8, max_retries: u32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            payload: Arc::new(payload),
            priority: priority.min(9), // Cap at 9
            retry_count: 0,
            max_retries,
            created_at: current_timestamp(),
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
/// Uses Arc to share message reference instead of cloning
#[derive(Debug, Clone)]
struct PendingMessage {
    message: Arc<QueueMessage>,
    #[allow(dead_code)]
    consumer_id: ConsumerId,
    ack_deadline: u32, // Unix timestamp for compact storage
}

impl PendingMessage {
    fn new(message: Arc<QueueMessage>, consumer_id: ConsumerId, ack_deadline_secs: u64) -> Self {
        let now = current_timestamp();
        Self {
            message,
            consumer_id,
            ack_deadline: now.saturating_add(ack_deadline_secs as u32),
        }
    }

    fn is_expired(&self) -> bool {
        current_timestamp() >= self.ack_deadline
    }
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
    messages: VecDeque<Arc<QueueMessage>>,
    pending: HashMap<MessageId, PendingMessage>,
    dead_letter: VecDeque<Arc<QueueMessage>>,
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
        let message_arc = Arc::new(message);

        // Insert in priority order (higher priority first)
        let insert_pos = self
            .messages
            .iter()
            .position(|m| m.priority < message_arc.priority)
            .unwrap_or(self.messages.len());

        self.messages.insert(insert_pos, message_arc);
        self.stats.published += 1;
        self.stats.depth = self.messages.len();

        Ok(message_id)
    }

    /// Consume message from queue
    fn consume(&mut self, consumer_id: ConsumerId) -> Option<QueueMessage> {
        if let Some(message_arc) = self.messages.pop_front() {
            let message_id = message_arc.id.clone();

            // Add to pending with Arc reference
            self.pending.insert(
                message_id,
                PendingMessage::new(
                    Arc::clone(&message_arc),
                    consumer_id,
                    self.config.ack_deadline_secs,
                ),
            );

            self.stats.consumed += 1;
            self.stats.depth = self.messages.len();
            self.stats.consumers = 1; // Simplified for now

            // Return cloned message (Arc deref + clone)
            Some((*message_arc).clone())
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
            // Get mutable copy of the message
            let message_ref = &pending.message;
            let mut message = (**message_ref).clone();

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
                self.dead_letter.push_back(Arc::new(message));
                self.stats.dead_lettered += 1;
            } else if requeue {
                // Requeue with updated retry count
                debug!(
                    "Requeuing message {} (retry {})",
                    message_id, message.retry_count
                );
                self.messages.push_back(Arc::new(message));
                self.stats.depth = self.messages.len();
            }

            Ok(())
        } else {
            Err(SynapError::MessageNotFound(message_id.to_string()))
        }
    }

    /// Check for expired pending messages
    fn check_expired_pending(&mut self) {
        let expired: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, p)| p.is_expired())
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

    /// Dump all queue messages for persistence
    pub async fn dump(&self) -> Result<HashMap<String, Vec<QueueMessage>>> {
        let queues = self.queues.read();
        let mut dump = HashMap::new();

        for (name, queue) in queues.iter() {
            let mut messages = Vec::new();
            // Clone the QueueMessage (not the Arc) for serialization
            messages.extend(queue.messages.iter().map(|arc_msg| (**arc_msg).clone()));
            // Note: Pending messages are in-flight, we could optionally include them
            dump.insert(name.clone(), messages);
        }

        Ok(dump)
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
        assert_eq!(*message.unwrap().payload, b"Hello");
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
        assert_eq!(*msg1.payload, b"High");
        assert_eq!(msg1.priority, 9);

        let msg2 = manager
            .consume("priority_queue", "c1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*msg2.payload, b"Medium");
        assert_eq!(msg2.priority, 5);

        let msg3 = manager
            .consume("priority_queue", "c1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*msg3.payload, b"Low");
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

    // ==================== CONCURRENCY TESTS ====================
    // These tests ensure no duplicate processing when multiple consumers
    // are competing for messages

    #[tokio::test]
    async fn test_concurrent_consumers_no_duplicates() {
        use std::collections::HashSet;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let manager = Arc::new(QueueManager::new(QueueConfig::default()));
        manager
            .create_queue("concurrent_queue", None)
            .await
            .unwrap();

        // Publish 100 messages
        let num_messages = 100;
        for i in 0..num_messages {
            manager
                .publish(
                    "concurrent_queue",
                    format!("msg-{}", i).into_bytes(),
                    None,
                    None,
                )
                .await
                .unwrap();
        }

        // Track consumed messages
        let consumed = Arc::new(Mutex::new(HashSet::new()));
        let mut handles = vec![];

        // Spawn 10 concurrent consumers
        for consumer_id in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let consumed_clone = Arc::clone(&consumed);

            let handle = tokio::spawn(async move {
                let consumer_name = format!("consumer-{}", consumer_id);

                // Each consumer tries to consume messages
                loop {
                    match manager_clone
                        .consume("concurrent_queue", &consumer_name)
                        .await
                    {
                        Ok(Some(msg)) => {
                            let mut set = consumed_clone.lock().await;
                            let message_content = String::from_utf8_lossy(&msg.payload).to_string();

                            // Check for duplicates - this should NEVER happen
                            assert!(
                                !set.contains(&message_content),
                                "DUPLICATE MESSAGE DETECTED: {} consumed by {}",
                                message_content,
                                consumer_name
                            );

                            set.insert(message_content.clone());
                            drop(set); // Release lock

                            // ACK the message
                            manager_clone
                                .ack("concurrent_queue", &msg.id)
                                .await
                                .unwrap();
                        }
                        Ok(None) => {
                            // Queue empty, we're done
                            break;
                        }
                        Err(e) => {
                            panic!("Unexpected error: {}", e);
                        }
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all consumers to finish
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all messages were consumed exactly once
        let final_consumed = consumed.lock().await;
        assert_eq!(
            final_consumed.len(),
            num_messages,
            "Expected {} messages, got {}",
            num_messages,
            final_consumed.len()
        );
    }

    #[tokio::test]
    async fn test_high_concurrency_stress_test() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let manager = Arc::new(QueueManager::new(QueueConfig::default()));
        manager.create_queue("stress_queue", None).await.unwrap();

        // Publish 1000 messages
        let num_messages = 1000;
        for i in 0..num_messages {
            manager
                .publish(
                    "stress_queue",
                    format!("msg-{:04}", i).into_bytes(),
                    None,
                    None,
                )
                .await
                .unwrap();
        }

        // Counter for consumed messages
        let consumed_count = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];

        // Spawn 50 concurrent consumers (high contention)
        for consumer_id in 0..50 {
            let manager_clone = Arc::clone(&manager);
            let counter_clone = Arc::clone(&consumed_count);

            let handle = tokio::spawn(async move {
                let consumer_name = format!("consumer-{}", consumer_id);
                let mut local_count = 0;

                loop {
                    match manager_clone.consume("stress_queue", &consumer_name).await {
                        Ok(Some(msg)) => {
                            local_count += 1;
                            counter_clone.fetch_add(1, Ordering::SeqCst);

                            // ACK immediately
                            manager_clone.ack("stress_queue", &msg.id).await.unwrap();
                        }
                        Ok(None) => {
                            break;
                        }
                        Err(e) => {
                            panic!("Unexpected error in consumer {}: {}", consumer_name, e);
                        }
                    }
                }

                local_count
            });

            handles.push(handle);
        }

        // Wait for all consumers and collect individual counts
        let mut total_from_consumers = 0;
        for handle in handles {
            let count = handle.await.unwrap();
            total_from_consumers += count;
        }

        // Verify counts match
        let final_count = consumed_count.load(Ordering::SeqCst);
        assert_eq!(
            final_count, num_messages,
            "Expected {} consumed messages, got {}",
            num_messages, final_count
        );
        assert_eq!(
            total_from_consumers, num_messages,
            "Sum of individual consumer counts ({}) doesn't match total ({})",
            total_from_consumers, num_messages
        );
    }

    #[tokio::test]
    async fn test_concurrent_publish_and_consume() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let manager = Arc::new(QueueManager::new(QueueConfig::default()));
        manager.create_queue("pubsub_queue", None).await.unwrap();

        let published_count = Arc::new(AtomicUsize::new(0));
        let consumed_count = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];

        // Spawn 5 publishers
        for publisher_id in 0..5 {
            let manager_clone = Arc::clone(&manager);
            let counter_clone = Arc::clone(&published_count);

            let handle = tokio::spawn(async move {
                for i in 0..100 {
                    let payload = format!("pub-{}-msg-{}", publisher_id, i).into_bytes();
                    manager_clone
                        .publish("pubsub_queue", payload, None, None)
                        .await
                        .unwrap();
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                }
            });

            handles.push(handle);
        }

        // Spawn 10 consumers (running concurrently with publishers)
        for consumer_id in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let counter_clone = Arc::clone(&consumed_count);

            let handle = tokio::spawn(async move {
                let consumer_name = format!("consumer-{}", consumer_id);

                // Keep consuming until we don't get messages for a while
                let mut empty_attempts = 0;
                while empty_attempts < 10 {
                    match manager_clone.consume("pubsub_queue", &consumer_name).await {
                        Ok(Some(msg)) => {
                            counter_clone.fetch_add(1, Ordering::SeqCst);
                            manager_clone.ack("pubsub_queue", &msg.id).await.unwrap();
                            empty_attempts = 0; // Reset
                        }
                        Ok(None) => {
                            empty_attempts += 1;
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                        Err(e) => {
                            panic!("Unexpected error: {}", e);
                        }
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Give a bit of time for final processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let published = published_count.load(Ordering::SeqCst);
        let consumed = consumed_count.load(Ordering::SeqCst);

        assert_eq!(published, 500, "Expected 500 published messages");
        assert_eq!(consumed, 500, "Expected all 500 messages to be consumed");
    }

    #[tokio::test]
    async fn test_no_message_loss_under_contention() {
        use std::collections::HashSet;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let manager = Arc::new(QueueManager::new(QueueConfig::default()));
        manager.create_queue("no_loss_queue", None).await.unwrap();

        // Publish 500 uniquely identifiable messages
        let num_messages = 500;
        let mut expected_messages = HashSet::new();

        for i in 0..num_messages {
            let msg_id = format!("unique-msg-{:05}", i);
            expected_messages.insert(msg_id.clone());
            manager
                .publish("no_loss_queue", msg_id.into_bytes(), None, None)
                .await
                .unwrap();
        }

        // Track received messages
        let received = Arc::new(Mutex::new(HashSet::new()));
        let mut handles = vec![];

        // Spawn 20 consumers with aggressive competition
        for consumer_id in 0..20 {
            let manager_clone = Arc::clone(&manager);
            let received_clone = Arc::clone(&received);

            let handle = tokio::spawn(async move {
                let consumer_name = format!("consumer-{}", consumer_id);

                loop {
                    match manager_clone.consume("no_loss_queue", &consumer_name).await {
                        Ok(Some(msg)) => {
                            let msg_content = String::from_utf8_lossy(&msg.payload).to_string();

                            let mut set = received_clone.lock().await;

                            // Detect duplicates
                            if set.contains(&msg_content) {
                                panic!("DUPLICATE: Message '{}' consumed twice!", msg_content);
                            }

                            set.insert(msg_content);
                            drop(set);

                            // ACK
                            manager_clone.ack("no_loss_queue", &msg.id).await.unwrap();
                        }
                        Ok(None) => break,
                        Err(e) => panic!("Error: {}", e),
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for completion
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all messages received exactly once
        let final_received = received.lock().await;

        assert_eq!(
            final_received.len(),
            num_messages,
            "Expected {} messages, received {}",
            num_messages,
            final_received.len()
        );

        // Verify we got exactly the messages we sent
        for expected in &expected_messages {
            assert!(
                final_received.contains(expected),
                "Message '{}' was never received!",
                expected
            );
        }
    }

    #[tokio::test]
    async fn test_priority_with_concurrent_consumers() {
        use std::sync::Arc;

        let manager = Arc::new(QueueManager::new(QueueConfig::default()));
        manager
            .create_queue("priority_concurrent", None)
            .await
            .unwrap();

        // Publish messages with different priorities
        for i in 0..30 {
            let priority = match i % 3 {
                0 => 9, // High
                1 => 5, // Medium
                _ => 1, // Low
            };
            manager
                .publish(
                    "priority_concurrent",
                    format!("msg-{}", i).into_bytes(),
                    Some(priority),
                    None,
                )
                .await
                .unwrap();
        }

        let mut handles = vec![];
        let manager_clone = Arc::clone(&manager);

        // Spawn 5 concurrent consumers
        for consumer_id in 0..5 {
            let manager_clone2 = Arc::clone(&manager_clone);

            let handle = tokio::spawn(async move {
                let consumer_name = format!("consumer-{}", consumer_id);
                let mut consumed = vec![];

                loop {
                    match manager_clone2
                        .consume("priority_concurrent", &consumer_name)
                        .await
                    {
                        Ok(Some(msg)) => {
                            consumed.push((
                                msg.priority,
                                String::from_utf8_lossy(&msg.payload).to_string(),
                            ));
                            manager_clone2
                                .ack("priority_concurrent", &msg.id)
                                .await
                                .unwrap();
                        }
                        Ok(None) => break,
                        Err(e) => panic!("Error: {}", e),
                    }
                }

                consumed
            });

            handles.push(handle);
        }

        // Collect all consumed messages
        let mut all_consumed = vec![];
        for handle in handles {
            let mut consumed = handle.await.unwrap();
            all_consumed.append(&mut consumed);
        }

        // Verify all 30 messages were consumed
        assert_eq!(
            all_consumed.len(),
            30,
            "All messages should be consumed exactly once"
        );

        // Verify higher priority messages tend to come first (not strict ordering due to concurrency)
        let high_priority_indices: Vec<usize> = all_consumed
            .iter()
            .enumerate()
            .filter(|(_, (prio, _))| *prio == 9)
            .map(|(idx, _)| idx)
            .collect();

        let low_priority_indices: Vec<usize> = all_consumed
            .iter()
            .enumerate()
            .filter(|(_, (prio, _))| *prio == 1)
            .map(|(idx, _)| idx)
            .collect();

        // On average, high priority should come before low priority
        if !high_priority_indices.is_empty() && !low_priority_indices.is_empty() {
            let avg_high: f64 = high_priority_indices.iter().sum::<usize>() as f64
                / high_priority_indices.len() as f64;
            let avg_low: f64 = low_priority_indices.iter().sum::<usize>() as f64
                / low_priority_indices.len() as f64;

            assert!(
                avg_high < avg_low,
                "High priority messages should generally come before low priority (avg high: {}, avg low: {})",
                avg_high,
                avg_low
            );
        }
    }
}
