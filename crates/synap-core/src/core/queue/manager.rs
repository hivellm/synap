//! Queue manager (the multi-queue store) over the per-queue `Queue` type.
//!
//! Split out of the former monolithic `queue.rs` (phase2 modularization).
//! `QueueMessage`, `QueueConfig`, `QueueStats` and the per-queue `Queue`
//! live in the parent module; this file holds the manager-level API.
use super::{MessageId, Queue, QueueConfig, QueueMessage, QueueState, QueueStats};
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

    /// Names of the queues holding an in-flight message whose ACK deadline
    /// has passed (possibly a stale entry; [`Self::requeue_expired`] skips
    /// those). Cheap: one heap peek per queue under the read lock.
    pub fn queues_with_expired_deliveries(&self) -> Vec<String> {
        let now = super::current_timestamp();
        self.queues
            .read()
            .iter()
            .filter(|(_, queue)| queue.has_expired_delivery(now))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// NACK (with requeue) every in-flight message of `queue_name` whose ACK
    /// deadline has passed, as the deadline checker does, and return their
    /// IDs so the caller can record each one as a NACK. A missing queue has
    /// nothing to requeue.
    pub fn requeue_expired(&self, queue_name: &str) -> Vec<MessageId> {
        match self.queues.write().get_mut(queue_name) {
            Some(queue) => queue.check_expired_pending(),
            None => Vec::new(),
        }
    }

    /// Create or get queue
    pub async fn create_queue(&self, name: &str, config: Option<QueueConfig>) -> Result<()> {
        self.create_queue_if_absent(name, config).await.map(|_| ())
    }

    /// Create the queue unless it exists. Returns the configuration it was
    /// created with, or `None` when a queue of that name already existed (its
    /// configuration is left unchanged).
    pub async fn create_queue_if_absent(
        &self,
        name: &str,
        config: Option<QueueConfig>,
    ) -> Result<Option<QueueConfig>> {
        debug!("Creating queue: {}", name);

        let mut queues = self.queues.write();
        if queues.contains_key(name) {
            return Ok(None);
        }
        let queue_config = config.unwrap_or_else(|| self.default_config.clone());
        queues.insert(
            name.to_string(),
            Queue::new(name.to_string(), queue_config.clone()),
        );

        Ok(Some(queue_config))
    }

    /// Insert `message` into `queue_name` exactly as it was published: same
    /// ID, payload, priority, retry counters, creation time and headers, at
    /// the position publishing gives it (behind every message of the same or
    /// higher priority).
    ///
    /// This is the restore primitive for WAL replay and replicas. The depth
    /// limit is not applied (the message was accepted when it was published),
    /// and the ID is taken as new: recovery and replication deliver each
    /// publish once (the snapshot barrier and replication offsets see to it).
    pub async fn restore_message(&self, queue_name: &str, message: QueueMessage) -> Result<()> {
        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;
        queue.insert_by_priority(message);
        Ok(())
    }

    /// Replay an ACK by message ID: the message is removed whether it is
    /// pending or still ready (deliveries are not logged, so on replay an
    /// acknowledged message is ready until its ACK arrives). Fails with
    /// `QueueNotFound` or `MessageNotFound` when there is nothing to remove.
    pub async fn replay_ack(&self, queue_name: &str, message_id: &str) -> Result<()> {
        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;
        queue.replay_ack(message_id)
    }

    /// Replay a NACK by message ID on a pending or ready message: its retry
    /// counter grows and it is requeued at the back, dead-lettered or dropped
    /// as the live NACK did. Fails with `QueueNotFound` or `MessageNotFound`
    /// when the message is not there.
    pub async fn replay_nack(
        &self,
        queue_name: &str,
        message_id: &str,
        requeue: bool,
    ) -> Result<()> {
        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;
        queue.replay_nack(message_id, requeue)
    }

    /// Purge `queue_name` (remove every ready message) and return the IDs it
    /// removed, so the purge can be replayed exactly.
    pub async fn purge_ids(&self, queue_name: &str) -> Result<Vec<MessageId>> {
        debug!("Purging queue: {}", queue_name);

        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;

        let ids = queue.messages.iter().map(|m| m.id.clone()).collect();
        queue.messages.clear();
        queue.stats.depth = 0;

        Ok(ids)
    }

    /// Replay a purge: remove the ready messages whose IDs are listed and
    /// return how many were removed. A purge only empties the ready list, and
    /// on replay delivered messages are still ready, so it is replayed by ID
    /// rather than by clearing the list.
    pub async fn remove_messages(
        &self,
        queue_name: &str,
        message_ids: &[MessageId],
    ) -> Result<usize> {
        let mut queues = self.queues.write();
        let queue = queues
            .get_mut(queue_name)
            .ok_or_else(|| SynapError::QueueNotFound(queue_name.to_string()))?;
        Ok(queue.remove_ready(message_ids))
    }

    /// Capture every queue with its configuration and every message (ready,
    /// in flight in delivery order, dead-lettered) with its full metadata.
    pub fn dump_state(&self) -> HashMap<String, QueueState> {
        self.queues
            .read()
            .iter()
            .map(|(name, queue)| (name.clone(), queue.state()))
            .collect()
    }

    /// Replace `name` with a queue rebuilt from `state` (see
    /// [`QueueState`]): every message keeps its ID and metadata, in-flight
    /// messages become ready again ahead of the waiting messages of the same
    /// or lower priority, in the order they were delivered.
    pub fn restore_queue(&self, name: &str, state: QueueState) {
        self.queues
            .write()
            .insert(name.to_string(), Queue::from_state(name.to_string(), state));
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
