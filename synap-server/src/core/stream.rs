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
use parking_lot::RwLock;
use uuid::Uuid;

/// Event stream configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Maximum messages per room buffer
    pub max_buffer_size: usize,
    /// Retention time in seconds (0 = infinite)
    pub retention_secs: u64,
    /// Enable automatic compaction
    pub auto_compact: bool,
    /// Compaction interval in seconds
    pub compact_interval_secs: u64,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: 10_000,  // 10K messages per room
            retention_secs: 3600,      // 1 hour default retention
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
}

/// Subscriber information
#[derive(Debug, Clone)]
struct Subscriber {
    id: String,
    last_offset: u64,
    last_active: std::time::Instant,
}

/// Single event stream room with ring buffer
struct Room {
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

        // Compact if needed
        while self.buffer.len() > self.config.max_buffer_size {
            if let Some(evt) = self.buffer.pop_front() {
                self.min_offset = evt.offset + 1;
            }
        }
        self.stats.message_count = self.buffer.len();
        self.stats.min_offset = self.min_offset;

        self.next_offset - 1
    }

    /// Consume events starting from an offset
    fn consume(&mut self, subscriber_id: &str, from_offset: u64, limit: usize) -> Vec<StreamEvent> {
        // Find events from offset
        let events: Vec<StreamEvent> = self.buffer
            .iter()
            .filter(|evt| evt.offset >= from_offset)
            .take(limit)
            .cloned()
            .collect();

        // Update subscriber after collecting events
        let last_offset = events.last().map(|e| e.offset + 1).unwrap_or(from_offset);
        
        let subscriber = self.subscribers
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

    /// Publish an event to a room
    pub async fn publish(
        &self,
        room: &str,
        event_type: &str,
        data: Vec<u8>,
    ) -> Result<u64, String> {
        let mut rooms = self.rooms.write();
        
        let room_obj = rooms.get_mut(room).ok_or_else(|| {
            format!("Room '{}' not found", room)
        })?;

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
        
        let room_obj = rooms.get_mut(room).ok_or_else(|| {
            format!("Room '{}' not found", room)
        })?;

        Ok(room_obj.consume(subscriber_id, from_offset, limit))
    }

    /// Get room statistics
    pub async fn room_stats(&self, room: &str) -> Result<RoomStats, String> {
        let rooms = self.rooms.read();
        
        rooms.get(room)
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

    /// Start background compaction task
    pub fn start_compaction_task(self: Arc<Self>) {
        if !self.config.auto_compact {
            return;
        }

        let interval_secs = self.config.compact_interval_secs;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(interval_secs)
            );

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
    async fn test_stream_publish_consume() {
        let manager = StreamManager::new(StreamConfig::default());
        manager.create_room("chat").await.unwrap();
        
        // Publish events
        let offset1 = manager.publish("chat", "message", b"Hello".to_vec()).await.unwrap();
        let offset2 = manager.publish("chat", "message", b"World".to_vec()).await.unwrap();
        
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
            manager.publish("events", "update", format!("event_{}", i).into_bytes())
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
            manager.publish("broadcast", "event", vec![i]).await.unwrap();
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
}

