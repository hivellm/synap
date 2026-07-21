//! Queue manager (the multi-queue store) over the per-queue `Queue` type.
//!
//! Split out of the former monolithic `queue.rs` (phase2 modularization).
//! `QueueMessage`, `QueueConfig`, `QueueStats` and the per-queue `Queue`
//! live in the parent module; this file holds the manager-level API.
use super::{MessageId, Queue, QueueConfig, QueueMessage, QueueStats};
use crate::core::error::{Result, SynapError};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// Queue manager (manages multiple queues)
#[derive(Clone)]
pub struct QueueManager {
    queues: Arc<RwLock<HashMap<String, Queue>>>,
    default_config: QueueConfig,
    /// Registered contribution to the shared cross-datatype budget (audit M-018).
    mem_bytes: Arc<std::sync::atomic::AtomicI64>,
    mem_attached: bool,
}

impl QueueManager {
    /// Create new queue manager
    pub fn new(config: QueueConfig) -> Self {
        info!("Initializing Queue Manager");
        Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
            default_config: config,
            mem_bytes: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            mem_attached: false,
        }
    }

    /// Attach the shared cross-datatype memory budget (audit M-018). Queues are
    /// already depth-capped, so they contribute to the accounted total (subject
    /// to eviction/refusal on other write paths) via `refresh_memory`.
    pub fn with_global_memory(mut self, mem: crate::core::GlobalMemory) -> Self {
        mem.register(Arc::clone(&self.mem_bytes));
        self.mem_attached = true;
        self
    }

    /// Total queued + dead-letter message-payload bytes across all queues.
    pub fn memory_bytes(&self) -> usize {
        let mut total = 0usize;
        for (name, q) in self.queues.read().iter() {
            total += name.len();
            for m in q.messages.iter() {
                total += m.payload.len();
            }
            for m in q.dead_letter.iter() {
                total += m.payload.len();
            }
        }
        total
    }

    /// Recompute this manager's accounted memory into its registered counter.
    pub fn refresh_memory(&self) {
        if self.mem_attached {
            self.mem_bytes.store(
                self.memory_bytes() as i64,
                std::sync::atomic::Ordering::Relaxed,
            );
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
        let message = self
            .publish_with_message(queue_name, payload, priority, max_retries)
            .await?;
        Ok(message.id)
    }

    /// Publish message to queue and return the full message
    /// This is useful for persistence logging
    pub async fn publish_with_message(
        &self,
        queue_name: &str,
        payload: Vec<u8>,
        priority: Option<u8>,
        max_retries: Option<u32>,
    ) -> Result<QueueMessage> {
        debug!("Publishing to queue: {}", queue_name);

        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;

        let priority = priority.unwrap_or(queue.config.default_priority);
        // If max_retries is not specified (None), use default. If specified (even as 0), use it.
        let max_retries = max_retries.unwrap_or(queue.config.default_max_retries);

        let message = QueueMessage::new(payload, priority, max_retries);
        let message_id = queue.publish(message.clone())?;

        // Verify message ID matches (should always be true)
        debug_assert_eq!(message.id, message_id);

        Ok(message)
    }

    /// Consume message from queue
    pub async fn consume(
        &self,
        queue_name: &str,
        consumer_id: &str,
    ) -> Result<Option<QueueMessage>> {
        debug!("Consuming from queue: {}", queue_name);

        let mut queues = self.queues.write();

        // ✅ FIX: Return Ok(None) instead of error when queue doesn't exist
        // This allows graceful handling of non-existent queues
        match queues.get_mut(queue_name) {
            Some(queue) => Ok(queue.consume(consumer_id.to_string())),
            None => {
                debug!("Queue not found: {}, returning None", queue_name);
                Ok(None)
            }
        }
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
