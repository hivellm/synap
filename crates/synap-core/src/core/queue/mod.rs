use super::error::{Result, SynapError};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;
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
    /// Per-consumer prefetch limit (QoS): the maximum number of unacked
    /// messages a single consumer may hold at once. A consumer already at its
    /// limit is not handed more messages until it acks, which also yields fair
    /// dispatch — those messages flow to other consumers instead. `0` means
    /// unlimited (default; preserves the previous unthrottled behavior).
    #[serde(default)]
    pub prefetch_limit: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_depth: 100_000,
            ack_deadline_secs: 30,
            default_max_retries: 3,
            default_priority: 5,
            prefetch_limit: 0,
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
    /// Min-heap of `(ack_deadline, message_id)` ordered by earliest deadline
    /// first. Lets the deadline checker inspect only entries that have actually
    /// expired instead of scanning every pending message. Entries are removed
    /// lazily: a popped entry is honored only if it still matches a live pending
    /// message with the same deadline (guards against acked/requeued messages).
    deadlines: BinaryHeap<Reverse<(u32, MessageId)>>,
    dead_letter: VecDeque<Arc<QueueMessage>>,
    /// Number of in-flight (unacked) messages per consumer. A consumer is
    /// "active" while it holds at least one such message; this backs an honest
    /// `stats.consumers` instead of the previous hardcoded 1.
    active_consumers: HashMap<ConsumerId, u32>,
    stats: QueueStats,
    config: QueueConfig,
}

impl Queue {
    fn new(name: String, config: QueueConfig) -> Self {
        Self {
            name,
            messages: VecDeque::new(),
            pending: HashMap::new(),
            deadlines: BinaryHeap::new(),
            dead_letter: VecDeque::new(),
            active_consumers: HashMap::new(),
            stats: QueueStats::default(),
            config,
        }
    }

    /// Decrement a consumer's in-flight count, dropping it from the active set
    /// when it reaches zero, and refresh `stats.consumers`.
    fn release_consumer(&mut self, consumer_id: &str) {
        if let Some(count) = self.active_consumers.get_mut(consumer_id) {
            *count -= 1;
            if *count == 0 {
                self.active_consumers.remove(consumer_id);
            }
        }
        self.stats.consumers = self.active_consumers.len();
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
        // Enforce per-consumer prefetch/QoS: a consumer already holding its limit
        // of unacked messages is throttled until it acks. This also produces fair
        // dispatch — while one consumer is at its limit, the pending messages are
        // available to other consumers instead of piling onto the fast one.
        if self.config.prefetch_limit > 0 {
            let in_flight = self
                .active_consumers
                .get(&consumer_id)
                .copied()
                .unwrap_or(0) as usize;
            if in_flight >= self.config.prefetch_limit {
                return None;
            }
        }

        if let Some(message_arc) = self.messages.pop_front() {
            let message_id = message_arc.id.clone();

            // Add to pending with Arc reference
            let pending = PendingMessage::new(
                Arc::clone(&message_arc),
                consumer_id.clone(),
                self.config.ack_deadline_secs,
            );
            let deadline = pending.ack_deadline;
            self.pending.insert(message_id.clone(), pending);
            // Track the deadline for O(expired) sweeping instead of O(pending).
            self.deadlines.push(Reverse((deadline, message_id)));
            // Mark the consumer active while it holds this unacked message.
            *self.active_consumers.entry(consumer_id).or_insert(0) += 1;

            self.stats.consumed += 1;
            self.stats.depth = self.messages.len();
            self.stats.consumers = self.active_consumers.len();

            // Return cloned message (Arc deref + clone)
            Some((*message_arc).clone())
        } else {
            None
        }
    }

    /// Acknowledge message
    fn ack(&mut self, message_id: &str) -> Result<()> {
        if let Some(pending) = self.pending.remove(message_id) {
            self.stats.acked += 1;
            self.release_consumer(&pending.consumer_id);
            Ok(())
        } else {
            Err(SynapError::MessageNotFound(message_id.to_string()))
        }
    }

    /// Negative acknowledge (requeue or dead letter)
    fn nack(&mut self, message_id: &str, requeue: bool) -> Result<()> {
        if let Some(pending) = self.pending.remove(message_id) {
            // The consumer no longer holds this message (requeued or dead-lettered).
            self.release_consumer(&pending.consumer_id);

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

    /// Check for expired pending messages.
    ///
    /// Pops the deadline heap only while the earliest deadline is in the past,
    /// so an idle sweep costs a single peek rather than a scan of every pending
    /// message. Stale heap entries (message already acked, or requeued and
    /// re-consumed with a fresh deadline) are discarded on pop.
    fn check_expired_pending(&mut self) {
        let now = current_timestamp();
        let mut to_requeue: Vec<MessageId> = Vec::new();

        while let Some(Reverse((deadline, _))) = self.deadlines.peek() {
            if *deadline > now {
                // Earliest deadline is still in the future — nothing expired.
                break;
            }
            let Reverse((deadline, id)) = self
                .deadlines
                .pop()
                .expect("peek returned Some in the loop condition");
            // Honor the expiry only if it still matches a live pending message
            // with this exact deadline; otherwise it is a stale entry.
            match self.pending.get(&id) {
                Some(p) if p.ack_deadline == deadline => to_requeue.push(id),
                _ => {}
            }
        }

        for message_id in to_requeue {
            debug!("Message {} ACK deadline expired, requeuing", message_id);
            let _ = self.nack(&message_id, true);
        }
    }
}

mod manager;
pub use manager::QueueManager;

#[cfg(test)]
mod tests;
