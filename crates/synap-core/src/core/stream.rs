use parking_lot::RwLock;
/// Event Stream module for Kafka-style room-based broadcasting
///
/// Features:
/// - Ring buffer for efficient message storage
/// - Room-based isolation (multi-tenant)
/// - Offset-based consumption with history replay
/// - Automatic compaction for old messages
/// - Subscriber tracking per room
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

/// Event stream configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Soft target size: the buffer is trimmed back toward this many events by
    /// evicting events the slowest tracked consumer has already read.
    pub max_buffer_size: usize,
    /// Hard ceiling on buffered events, including those still unread by the
    /// slowest tracked consumer. The buffer is allowed to grow past
    /// `max_buffer_size` (up to this cap) rather than silently drop unread
    /// events; only when it would exceed this cap are unread events shed, and
    /// that loss is surfaced via `RoomStats.dropped` (audit M-012). Values
    /// below `max_buffer_size` are treated as `max_buffer_size` (plain ring
    /// buffer). With no subscribers there is nothing to protect, so the buffer
    /// behaves as a ring buffer at `max_buffer_size`.
    #[serde(default = "default_max_unread_buffer_size")]
    pub max_unread_buffer_size: usize,
    /// Retention time in seconds (0 = infinite)
    pub retention_secs: u64,
    /// Enable automatic compaction
    pub auto_compact: bool,
    /// Compaction interval in seconds
    pub compact_interval_secs: u64,
}

fn default_max_unread_buffer_size() -> usize {
    100_000 // 10x the default soft buffer: a wide consumer-lag window, still bounded
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: 10_000, // 10K messages per room
            max_unread_buffer_size: default_max_unread_buffer_size(),
            retention_secs: 3600, // 1 hour default retention
            auto_compact: true,
            compact_interval_secs: 60, // Compact every minute
        }
    }
}

/// Event message in a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Unique event ID
    pub id: String,
    /// Event offset in the stream (sequential)
    pub offset: u64,
    /// Room/channel name
    pub room: String,
    /// Event type/name
    pub event: String,
    /// Event data (JSON or bytes)
    pub data: Vec<u8>,
    /// Unix timestamp when created
    pub timestamp: u64,
    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl StreamEvent {
    /// Create a new stream event
    pub fn new(room: String, event: String, data: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            offset: 0, // Will be set by room
            room,
            event,
            data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        }
    }
}

/// Room statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomStats {
    /// Room name
    pub name: String,
    /// Total messages in buffer
    pub message_count: usize,
    /// Oldest offset available
    pub min_offset: u64,
    /// Latest offset
    pub max_offset: u64,
    /// Number of active subscribers
    pub subscriber_count: usize,
    /// Total messages published (all time)
    pub total_published: u64,
    /// Total messages consumed (all time)
    pub total_consumed: u64,
    /// Events evicted from the ring buffer while still unread by the slowest
    /// tracked subscriber (audit M-012). Normal recycling of already-consumed
    /// events is not counted; a non-zero value means a lagging consumer lost
    /// data. Full durability (disk spill) is tracked separately in phase6f.
    #[serde(default)]
    pub dropped: u64,
}

/// Subscriber information
#[derive(Debug, Clone)]
struct Subscriber {
    #[allow(dead_code)]
    id: String,
    last_offset: u64,
    last_active: std::time::Instant,
}

/// Single event stream room with ring buffer
struct Room {
    #[allow(dead_code)]
    name: String,
    /// Ring buffer for messages (FIFO with max size)
    buffer: VecDeque<StreamEvent>,
    /// Next offset to assign
    next_offset: u64,
    /// Minimum offset available (after compaction)
    min_offset: u64,
    /// Active subscribers
    subscribers: HashMap<String, Subscriber>,
    /// Room statistics
    stats: RoomStats,
    /// Configuration
    config: StreamConfig,
}

impl Room {
    fn new(name: String, config: StreamConfig) -> Self {
        Self {
            stats: RoomStats {
                name: name.clone(),
                message_count: 0,
                min_offset: 0,
                max_offset: 0,
                subscriber_count: 0,
                total_published: 0,
                total_consumed: 0,
                dropped: 0,
            },
            name,
            buffer: VecDeque::with_capacity(config.max_buffer_size),
            next_offset: 0,
            min_offset: 0,
            subscribers: HashMap::new(),
            config,
        }
    }

    /// Publish an event to the room
    fn publish(&mut self, mut event: StreamEvent) -> u64 {
        // Assign offset
        event.offset = self.next_offset;
        self.next_offset += 1;

        // Add to buffer
        self.buffer.push_back(event);
        self.stats.total_published += 1;
        self.stats.message_count = self.buffer.len();
        self.stats.max_offset = self.next_offset - 1;

        // Retention (audit M-012): trim back toward `max_buffer_size` by evicting
        // events the slowest tracked consumer has already read. Events it has not
        // read yet (offset >= its committed offset) are protected — the buffer is
        // allowed to grow up to `max_unread_buffer_size` rather than lose them
        // silently. Only when it would exceed that hard cap is an unread event
        // shed, and that real loss is counted in `stats.dropped`. With no tracked
        // subscribers there is nothing to protect: plain ring buffer.
        let slowest_committed = self.subscribers.values().map(|s| s.last_offset).min();
        let hard_cap = self
            .config
            .max_unread_buffer_size
            .max(self.config.max_buffer_size);
        while self.buffer.len() > self.config.max_buffer_size {
            let oldest_offset = match self.buffer.front() {
                Some(evt) => evt.offset,
                None => break,
            };
            let is_unread = slowest_committed.is_some_and(|c| oldest_offset >= c);
            if is_unread && self.buffer.len() <= hard_cap {
                // Protect unread data; let the buffer grow (bounded by hard_cap).
                break;
            }
            if let Some(evt) = self.buffer.pop_front() {
                if is_unread {
                    // Forced drop of unread data at the hard cap — real loss.
                    self.stats.dropped += 1;
                }
                self.min_offset = evt.offset + 1;
            }
        }
        self.stats.message_count = self.buffer.len();
        self.stats.min_offset = self.min_offset;

        self.next_offset - 1
    }

    /// Consume events starting from an offset
    fn consume(&mut self, subscriber_id: &str, from_offset: u64, limit: usize) -> Vec<StreamEvent> {
        // Offsets are contiguous in the ring buffer (buffer[i].offset == min_offset + i),
        // so seek straight to the first requested index instead of scanning the whole
        // buffer (audit M-016: O(limit) rather than O(buffer)).
        let start_idx =
            (from_offset.saturating_sub(self.min_offset) as usize).min(self.buffer.len());
        let events: Vec<StreamEvent> = self
            .buffer
            .range(start_idx..)
            .take(limit)
            .cloned()
            .collect();

        // Update subscriber after collecting events
        let last_offset = events.last().map(|e| e.offset + 1).unwrap_or(from_offset);

        let subscriber = self
            .subscribers
            .entry(subscriber_id.to_string())
            .or_insert_with(|| Subscriber {
                id: subscriber_id.to_string(),
                last_offset: from_offset,
                last_active: std::time::Instant::now(),
            });

        subscriber.last_active = std::time::Instant::now();
        subscriber.last_offset = last_offset;

        self.stats.subscriber_count = self.subscribers.len();
        self.stats.total_consumed += events.len() as u64;

        events
    }

    /// Get room statistics
    fn stats(&self) -> RoomStats {
        self.stats.clone()
    }

    /// Compact old messages based on retention policy
    fn compact(&mut self) {
        if !self.config.auto_compact {
            return;
        }

        let cutoff_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(self.config.retention_secs);

        let initial_len = self.buffer.len();
        self.buffer.retain(|evt| evt.timestamp > cutoff_time);

        let removed = initial_len - self.buffer.len();
        if removed > 0 && !self.buffer.is_empty() {
            if let Some(first) = self.buffer.front() {
                self.min_offset = first.offset;
            }
            self.stats.message_count = self.buffer.len();
        }
    }
}

/// Event Stream Manager
#[derive(Clone)]
pub struct StreamManager {
    rooms: Arc<RwLock<HashMap<String, Room>>>,
    config: StreamConfig,
}

impl StreamManager {
    /// Create a new stream manager
    pub fn new(config: StreamConfig) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Create a new room
    pub async fn create_room(&self, room_name: &str) -> Result<(), String> {
        let mut rooms = self.rooms.write();

        if rooms.contains_key(room_name) {
            return Err(format!("Room '{}' already exists", room_name));
        }

        rooms.insert(
            room_name.to_string(),
            Room::new(room_name.to_string(), self.config.clone()),
        );

        Ok(())
    }

    /// Get a room or create it if it does not exist.
    ///
    /// Idempotent: the second concurrent caller observes the
    /// already-created room and returns `false` for `created`,
    /// instead of erroring like [`Self::create_room`] does.
    ///
    /// Returns `true` if a new room was created by this call,
    /// `false` if the room already existed.
    pub async fn get_or_create_room(&self, room_name: &str) -> Result<bool, String> {
        let mut rooms = self.rooms.write();

        if rooms.contains_key(room_name) {
            return Ok(false);
        }

        rooms.insert(
            room_name.to_string(),
            Room::new(room_name.to_string(), self.config.clone()),
        );

        Ok(true)
    }

    /// Publish an event to a room
    pub async fn publish(
        &self,
        room: &str,
        event_type: &str,
        data: Vec<u8>,
    ) -> Result<u64, String> {
        let mut rooms = self.rooms.write();

        let room_obj = rooms
            .get_mut(room)
            .ok_or_else(|| format!("Room '{}' not found", room))?;

        let event = StreamEvent::new(room.to_string(), event_type.to_string(), data);
        let offset = room_obj.publish(event);

        Ok(offset)
    }

    /// Consume events from a room
    pub async fn consume(
        &self,
        room: &str,
        subscriber_id: &str,
        from_offset: u64,
        limit: usize,
    ) -> Result<Vec<StreamEvent>, String> {
        let mut rooms = self.rooms.write();

        let room_obj = rooms
            .get_mut(room)
            .ok_or_else(|| format!("Room '{}' not found", room))?;

        Ok(room_obj.consume(subscriber_id, from_offset, limit))
    }

    /// Get room statistics
    pub async fn room_stats(&self, room: &str) -> Result<RoomStats, String> {
        let rooms = self.rooms.read();

        rooms
            .get(room)
            .map(|r| r.stats())
            .ok_or_else(|| format!("Room '{}' not found", room))
    }

    /// List all rooms
    pub async fn list_rooms(&self) -> Vec<String> {
        let rooms = self.rooms.read();
        rooms.keys().cloned().collect()
    }

    /// Delete a room
    pub async fn delete_room(&self, room: &str) -> Result<(), String> {
        let mut rooms = self.rooms.write();

        if rooms.remove(room).is_some() {
            Ok(())
        } else {
            Err(format!("Room '{}' not found", room))
        }
    }

    /// Get all events from all rooms (for snapshot)
    pub async fn get_all_events(&self) -> HashMap<String, Vec<StreamEvent>> {
        let rooms = self.rooms.read();
        let mut all_events = HashMap::new();

        for (room_name, room) in rooms.iter() {
            let events: Vec<StreamEvent> = room.buffer.iter().cloned().collect();
            if !events.is_empty() {
                all_events.insert(room_name.clone(), events);
            }
        }

        all_events
    }

    /// Restore a room from snapshot (for replication)
    pub async fn restore_room(
        &self,
        room_name: &str,
        events: Vec<StreamEvent>,
    ) -> Result<(), String> {
        // Create room if it doesn't exist
        let _ = self.create_room(room_name).await;

        // Publish events
        for event in events {
            self.publish(room_name, &event.event, event.data).await?;
        }

        Ok(())
    }

    /// Start background compaction task
    pub fn start_compaction_task(self: Arc<Self>) {
        if !self.config.auto_compact {
            return;
        }

        let interval_secs = self.config.compact_interval_secs;

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                let mut rooms = self.rooms.write();
                for room in rooms.values_mut() {
                    room.compact();
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stream_create_room() {
        let manager = StreamManager::new(StreamConfig::default());

        manager.create_room("test-room").await.unwrap();

        let rooms = manager.list_rooms().await;
        assert_eq!(rooms.len(), 1);
        assert!(rooms.contains(&"test-room".to_string()));
    }

    #[tokio::test]
    async fn test_stream_get_or_create_room_is_idempotent() {
        let manager = StreamManager::new(StreamConfig::default());

        // First call creates the room.
        let created = manager.get_or_create_room("idempotent").await.unwrap();
        assert!(created, "first call should report the room as created");

        // Second call must NOT error and must report the room as
        // already-existing (closes hivellm/synap#165).
        let created_again = manager.get_or_create_room("idempotent").await.unwrap();
        assert!(
            !created_again,
            "second call should observe the existing room without erroring"
        );

        // The room must be usable immediately after creation, with no
        // "Room not found" first-publish failure.
        let offset = manager
            .publish("idempotent", "evt", b"first".to_vec())
            .await
            .unwrap();
        assert_eq!(offset, 0);

        let rooms = manager.list_rooms().await;
        assert_eq!(rooms.len(), 1);
    }

    #[tokio::test]
    async fn test_stream_get_or_create_after_create_room_does_not_error() {
        let manager = StreamManager::new(StreamConfig::default());

        manager.create_room("mixed").await.unwrap();
        let created = manager.get_or_create_room("mixed").await.unwrap();
        assert!(!created);
    }

    #[tokio::test]
    async fn test_stream_publish_consume() {
        let manager = StreamManager::new(StreamConfig::default());
        manager.create_room("chat").await.unwrap();

        // Publish events
        let offset1 = manager
            .publish("chat", "message", b"Hello".to_vec())
            .await
            .unwrap();
        let offset2 = manager
            .publish("chat", "message", b"World".to_vec())
            .await
            .unwrap();

        assert_eq!(offset1, 0);
        assert_eq!(offset2, 1);

        // Consume from offset 0
        let events = manager.consume("chat", "subscriber1", 0, 10).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, b"Hello");
        assert_eq!(events[1].data, b"World");
        assert_eq!(events[0].offset, 0);
        assert_eq!(events[1].offset, 1);
    }

    #[tokio::test]
    async fn test_stream_offset_tracking() {
        let manager = StreamManager::new(StreamConfig::default());
        manager.create_room("events").await.unwrap();

        // Publish 5 events
        for i in 0..5 {
            manager
                .publish("events", "update", format!("event_{}", i).into_bytes())
                .await
                .unwrap();
        }

        // Consume from offset 2
        let events = manager.consume("events", "sub1", 2, 10).await.unwrap();
        assert_eq!(events.len(), 3); // Offsets 2, 3, 4
        assert_eq!(events[0].offset, 2);

        // Consume from offset 4
        let events = manager.consume("events", "sub2", 4, 10).await.unwrap();
        assert_eq!(events.len(), 1); // Only offset 4
        assert_eq!(events[0].offset, 4);
    }

    #[tokio::test]
    async fn test_stream_ring_buffer_overflow() {
        let mut config = StreamConfig::default();
        config.max_buffer_size = 5; // Small buffer for testing

        let manager = StreamManager::new(config);
        manager.create_room("limited").await.unwrap();

        // Publish 10 events (will overflow)
        for i in 0..10 {
            manager.publish("limited", "msg", vec![i]).await.unwrap();
        }

        // Should only have last 5 messages
        let stats = manager.room_stats("limited").await.unwrap();
        assert_eq!(stats.message_count, 5);
        assert_eq!(stats.min_offset, 5); // First 5 were dropped
        assert_eq!(stats.max_offset, 9); // Last offset is 9

        // Consume from offset 5 (oldest available)
        let events = manager.consume("limited", "sub", 5, 10).await.unwrap();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].offset, 5);
        assert_eq!(events[4].offset, 9);
    }

    #[tokio::test]
    async fn test_stream_multiple_subscribers() {
        let manager = StreamManager::new(StreamConfig::default());
        manager.create_room("broadcast").await.unwrap();

        // Publish events
        for i in 0..5 {
            manager
                .publish("broadcast", "event", vec![i])
                .await
                .unwrap();
        }

        // Multiple subscribers at different offsets
        let events1 = manager.consume("broadcast", "sub1", 0, 10).await.unwrap();
        let events2 = manager.consume("broadcast", "sub2", 3, 10).await.unwrap();
        let events3 = manager.consume("broadcast", "sub3", 5, 10).await.unwrap();

        assert_eq!(events1.len(), 5); // sub1 gets all
        assert_eq!(events2.len(), 2); // sub2 gets offsets 3,4
        assert_eq!(events3.len(), 0); // sub3 gets nothing (offset 5 doesn't exist yet)

        let stats = manager.room_stats("broadcast").await.unwrap();
        assert_eq!(stats.subscriber_count, 3);
    }

    // ==================== DROP ACCOUNTING TESTS (M-012) ====================

    fn room_with(max_buffer_size: usize, max_unread_buffer_size: usize) -> Room {
        Room::new(
            "r".to_string(),
            StreamConfig {
                max_buffer_size,
                max_unread_buffer_size,
                retention_secs: 0,
                auto_compact: false,
                compact_interval_secs: 60,
            },
        )
    }

    // Plain ring buffer: hard cap == soft size, so unread events are not protected.
    fn small_room() -> Room {
        room_with(2, 2)
    }

    #[test]
    fn test_stream_no_drop_without_subscribers() {
        // With no tracked subscriber, ring-buffer recycling is not data loss.
        let mut room = small_room();
        for i in 0..5 {
            room.publish(StreamEvent::new("r".to_string(), "e".to_string(), vec![i]));
        }
        assert_eq!(room.buffer.len(), 2); // capacity enforced
        assert_eq!(room.stats().dropped, 0); // but nothing counted as lost
    }

    #[test]
    fn test_stream_counts_unread_eviction_for_lagging_subscriber() {
        let mut room = small_room();

        // Publish e0, then a subscriber reads only up to offset 0 (last_offset=1).
        room.publish(StreamEvent::new("r".to_string(), "e".to_string(), vec![0]));
        let got = room.consume("s", 0, 1);
        assert_eq!(got.len(), 1);

        // Publish e1..e4 without the subscriber catching up. Evictions of events
        // at offset >= 1 (unread by "s") count as loss; evicting the already-read
        // offset 0 does not.
        for i in 1..=4 {
            room.publish(StreamEvent::new("r".to_string(), "e".to_string(), vec![i]));
        }

        // Evicted: off0 (read, not counted), off1 and off2 (unread) -> dropped == 2.
        assert_eq!(room.stats().dropped, 2);
        assert_eq!(room.buffer.len(), 2);
    }

    #[test]
    fn test_stream_retains_unread_up_to_hard_cap() {
        // Soft size 2, hard cap 5: unread events are protected (buffer grows to 5)
        // instead of being dropped, until the hard cap forces a single eviction.
        let mut room = room_with(2, 5);

        // Publish e0, subscriber reads only up to offset 0 (committed = 1).
        room.publish(StreamEvent::new("r".to_string(), "e".to_string(), vec![0]));
        assert_eq!(room.consume("s", 0, 1).len(), 1);

        // Publish e1..e6 without the subscriber catching up.
        for i in 1..=6u8 {
            room.publish(StreamEvent::new("r".to_string(), "e".to_string(), vec![i]));
        }

        // Read e0 is evicted (not counted); unread events are retained up to the
        // hard cap of 5, and exactly one unread event (e1) is shed when the 6th
        // unread event would exceed the cap.
        assert_eq!(room.buffer.len(), 5);
        assert_eq!(room.stats().dropped, 1);
        // The oldest retained event is e2 (e0 recycled, e1 dropped at the cap).
        assert_eq!(room.buffer.front().unwrap().offset, 2);
        assert_eq!(room.stats().min_offset, 2);
    }

    #[test]
    fn test_stream_no_protection_without_subscribers_even_with_hard_cap() {
        // A wide hard cap must not make an unsubscribed room grow: nobody to
        // protect, so it stays a ring buffer at the soft size.
        let mut room = room_with(2, 100);
        for i in 0..10u8 {
            room.publish(StreamEvent::new("r".to_string(), "e".to_string(), vec![i]));
        }
        assert_eq!(room.buffer.len(), 2);
        assert_eq!(room.stats().dropped, 0);
    }
}
