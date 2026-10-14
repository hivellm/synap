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
    /// Position of this delivery in the queue's delivery order, so a snapshot
    /// can list in-flight messages in the order they were delivered.
    delivery_seq: u64,
}

impl PendingMessage {
    fn new(
        message: Arc<QueueMessage>,
        consumer_id: ConsumerId,
        ack_deadline_secs: u64,
        delivery_seq: u64,
    ) -> Self {
        let now = current_timestamp();
        Self {
            message,
            consumer_id,
            ack_deadline: now.saturating_add(ack_deadline_secs as u32),
            delivery_seq,
        }
    }
}

/// Everything one queue holds, as a snapshot captures it and recovery
/// restores it: its configuration and every message with its full metadata
/// (ID, priority, retry counters, creation time, headers).
#[derive(Debug, Clone)]
pub struct QueueState {
    /// The queue's configuration.
    pub config: QueueConfig,
    /// Messages waiting for delivery, in delivery order.
    pub ready: Vec<QueueMessage>,
    /// Messages delivered but not acknowledged, in the order they were
    /// delivered. No consumer survives a restart, so recovery makes them
    /// ready again (see [`QueueManager::restore_queue`]).
    pub in_flight: Vec<QueueMessage>,
    /// Dead-lettered messages, oldest first.
    pub dead_letter: Vec<QueueMessage>,
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
    /// Sequence number handed to the next delivery (see `PendingMessage`).
    next_delivery_seq: u64,
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
            next_delivery_seq: 0,
        }
    }

    /// Rebuild a queue from a captured [`QueueState`].
    ///
    /// Ready and dead-lettered messages keep their captured order. In-flight
    /// messages become ready again (their consumers did not survive the
    /// restart): each goes ahead of the waiting messages of the same or lower
    /// priority, and they keep the order they were delivered in, so they are
    /// redelivered before the messages that were still waiting behind them.
    fn from_state(name: String, state: QueueState) -> Self {
        let QueueState {
            config,
            ready,
            in_flight,
            dead_letter,
        } = state;
        let mut queue = Self::new(name, config);
        queue.messages = ready.into_iter().map(Arc::new).collect();
        for message in in_flight.into_iter().rev() {
            let position = queue
                .messages
                .iter()
                .position(|m| m.priority <= message.priority)
                .unwrap_or(queue.messages.len());
            queue.messages.insert(position, Arc::new(message));
        }
        queue.dead_letter = dead_letter.into_iter().map(Arc::new).collect();
        queue.stats.depth = queue.messages.len();
        queue
    }

    /// Capture this queue as a [`QueueState`] (in-flight messages in delivery
    /// order).
    fn state(&self) -> QueueState {
        let mut in_flight: Vec<&PendingMessage> = self.pending.values().collect();
        in_flight.sort_unstable_by_key(|p| p.delivery_seq);
        QueueState {
            config: self.config.clone(),
            ready: self.messages.iter().map(|m| (**m).clone()).collect(),
            in_flight: in_flight.iter().map(|p| (*p.message).clone()).collect(),
            dead_letter: self.dead_letter.iter().map(|m| (**m).clone()).collect(),
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
        self.insert_by_priority(message);
        Ok(message_id)
    }

    /// Insert a message that was already accepted, exactly as it is (ID,
    /// priority, retry counters, timestamps, headers), where publishing puts
    /// it: behind every message of the same or higher priority.
    ///
    /// The depth limit is not checked: the message was within it when it was
    /// published, and a replay can hold more ready messages than the live
    /// queue did, because deliveries are not logged and delivered messages
    /// stay ready until their ACK is replayed.
    fn insert_by_priority(&mut self, message: QueueMessage) {
        let insert_pos = self
            .messages
            .iter()
            .position(|m| m.priority < message.priority)
            .unwrap_or(self.messages.len());

        self.messages.insert(insert_pos, Arc::new(message));
        self.stats.published += 1;
        self.stats.depth = self.messages.len();
    }

    /// Remove a ready (not yet delivered) message by ID.
    fn take_ready(&mut self, message_id: &str) -> Option<Arc<QueueMessage>> {
        let position = self.messages.iter().position(|m| m.id == message_id)?;
        let message = self.messages.remove(position);
        self.stats.depth = self.messages.len();
        message
    }

    /// Replay an ACK: the message is removed whether it is pending (delivered)
    /// or still ready. Recovery and replicas never see deliveries (they are
    /// not logged), so an acknowledged message is ready there until its ACK
    /// is replayed.
    fn replay_ack(&mut self, message_id: &str) -> Result<()> {
        if self.pending.contains_key(message_id) {
            return self.ack(message_id);
        }
        match self.take_ready(message_id) {
            Some(_) => {
                self.stats.acked += 1;
                Ok(())
            }
            None => Err(SynapError::MessageNotFound(message_id.to_string())),
        }
    }

    /// Replay a NACK on a pending or ready message (see [`Self::replay_ack`]):
    /// the retry counter grows and the message is requeued at the back,
    /// dead-lettered or dropped, exactly as the live NACK did.
    fn replay_nack(&mut self, message_id: &str, requeue: bool) -> Result<()> {
        if self.pending.contains_key(message_id) {
            return self.nack(message_id, requeue);
        }
        match self.take_ready(message_id) {
            Some(message) => {
                self.stats.nacked += 1;
                self.settle_nack((*message).clone(), requeue);
                Ok(())
            }
            None => Err(SynapError::MessageNotFound(message_id.to_string())),
        }
    }

    /// Remove the ready messages whose IDs are in `message_ids` (a replayed
    /// purge); returns how many were removed.
    fn remove_ready(&mut self, message_ids: &[MessageId]) -> usize {
        let ids: std::collections::HashSet<&str> = message_ids.iter().map(String::as_str).collect();
        let before = self.messages.len();
        self.messages.retain(|m| !ids.contains(m.id.as_str()));
        self.stats.depth = self.messages.len();
        before - self.messages.len()
    }

    /// Whether the earliest in-flight ACK deadline has passed. The entry may
    /// be stale (already acknowledged); [`Self::check_expired_pending`]
    /// discards those.
    fn has_expired_delivery(&self, now: u32) -> bool {
        self.deadlines
            .peek()
            .is_some_and(|Reverse((deadline, _))| *deadline <= now)
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
            let delivery_seq = self.next_delivery_seq;
            self.next_delivery_seq += 1;
            let pending = PendingMessage::new(
                Arc::clone(&message_arc),
                consumer_id.clone(),
                self.config.ack_deadline_secs,
                delivery_seq,
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

            self.stats.nacked += 1;
            self.settle_nack((*pending.message).clone(), requeue);

            Ok(())
        } else {
            Err(SynapError::MessageNotFound(message_id.to_string()))
        }
    }

    /// The outcome of a NACK for a message already taken out of the queue:
    /// its retry counter grows, then it is dead-lettered once it exceeds its
    /// retries, requeued at the back when `requeue` is set, or dropped.
    fn settle_nack(&mut self, mut message: QueueMessage, requeue: bool) {
        // Increment retry count first
        message.increment_retry();

        // Then check if exceeded max retries
        if message.is_dead() {
            // Move to dead letter queue
            debug!(
                "Message {} exceeded retries (retry_count={}, max={}), moving to DLQ",
                message.id, message.retry_count, message.max_retries
            );
            self.dead_letter.push_back(Arc::new(message));
            self.stats.dead_lettered += 1;
        } else if requeue {
            // Requeue with updated retry count
            debug!(
                "Requeuing message {} (retry {})",
                message.id, message.retry_count
            );
            self.messages.push_back(Arc::new(message));
            self.stats.depth = self.messages.len();
        }
    }

    /// Check for expired pending messages.
    ///
    /// Pops the deadline heap only while the earliest deadline is in the past,
    /// so an idle sweep costs a single peek rather than a scan of every pending
    /// message. Stale heap entries (message already acked, or requeued and
    /// re-consumed with a fresh deadline) are discarded on pop. Returns the
    /// IDs of the messages it NACKed (requeued or dead-lettered), so the
    /// caller can record each one as the NACK it is.
    fn check_expired_pending(&mut self) -> Vec<MessageId> {
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

        for message_id in &to_requeue {
            debug!("Message {} ACK deadline expired, requeuing", message_id);
            // Every ID was just checked against `pending`, so the NACK finds it.
            let _ = self.nack(message_id, true);
        }
        to_requeue
    }
}

mod manager;
pub use manager::QueueManager;

#[cfg(test)]
mod tests;
