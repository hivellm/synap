use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Retention policy types (Kafka-style)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RetentionPolicy {
    /// Retain messages for a specific duration
    Time {
        /// Retention time in seconds
        retention_secs: u64,
    },
    /// Retain up to a specific size in bytes
    Size {
        /// Maximum size in bytes
        max_bytes: u64,
    },
    /// Retain up to a specific number of messages
    Messages {
        /// Maximum number of messages
        max_messages: u64,
    },
    /// Combined policy (smallest limit wins)
    Combined {
        retention_secs: Option<u64>,
        max_bytes: Option<u64>,
        max_messages: Option<u64>,
    },
    /// Infinite retention (never delete)
    Infinite,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        RetentionPolicy::Time {
            retention_secs: 3600, // 1 hour
        }
    }
}

/// Partition configuration (Kafka-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionConfig {
    /// Number of partitions for this topic/room
    pub num_partitions: usize,
    /// Replication factor (for future replication)
    pub replication_factor: usize,
    /// Retention policy
    pub retention: RetentionPolicy,
    /// Segment size for log rotation (bytes)
    pub segment_bytes: u64,
    /// Maximum batch size for writes
    pub max_batch_size: usize,
    /// Compression enabled
    pub compression_enabled: bool,
    /// Flush interval in seconds
    pub flush_interval_secs: u64,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            num_partitions: 3,
            replication_factor: 1,
            retention: RetentionPolicy::default(),
            segment_bytes: 100 * 1024 * 1024, // 100MB segments
            max_batch_size: 1000,
            compression_enabled: false,
            flush_interval_secs: 5,
        }
    }
}

/// Event in a partition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionEvent {
    /// Unique event ID
    pub id: String,
    /// Partition ID
    pub partition_id: usize,
    /// Offset within partition
    pub offset: u64,
    /// Topic/Room name
    pub topic: String,
    /// Event type
    pub event_type: String,
    /// Key for partitioning (optional)
    pub key: Option<Vec<u8>>,
    /// Event data
    pub data: Vec<u8>,
    /// Timestamp
    pub timestamp: u64,
    /// Size in bytes (for retention calculation)
    pub size_bytes: u64,
    /// Metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl PartitionEvent {
    pub fn new(topic: String, event_type: String, key: Option<Vec<u8>>, data: Vec<u8>) -> Self {
        let size_bytes = data.len() as u64
            + key.as_ref().map(|k| k.len()).unwrap_or(0) as u64
            + topic.len() as u64
            + event_type.len() as u64;

        Self {
            id: Uuid::new_v4().to_string(),
            partition_id: 0, // Will be set later
            offset: 0,       // Will be set later
            topic,
            event_type,
            key,
            data,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            size_bytes,
            metadata: HashMap::new(),
        }
    }
}

/// Single partition within a topic
pub struct Partition {
    id: usize,
    topic: String,
    /// Ring buffer for messages
    buffer: VecDeque<PartitionEvent>,
    /// Next offset to assign
    next_offset: u64,
    /// Minimum offset available
    min_offset: u64,
    /// Total bytes in partition
    total_bytes: u64,
    /// Configuration
    config: PartitionConfig,
    /// Last compaction time
    last_compaction: Instant,
}

impl Partition {
    pub fn new(id: usize, topic: String, config: PartitionConfig) -> Self {
        Self {
            id,
            topic,
            buffer: VecDeque::new(),
            next_offset: 0,
            min_offset: 0,
            total_bytes: 0,
            config,
            last_compaction: Instant::now(),
        }
    }

    /// Append event to partition
    pub fn append(&mut self, mut event: PartitionEvent) -> u64 {
        event.partition_id = self.id;
        event.offset = self.next_offset;
        
        self.total_bytes += event.size_bytes;
        self.buffer.push_back(event);
        self.next_offset += 1;

        // Auto-compact if needed
        self.maybe_compact();

        self.next_offset - 1
    }

    /// Read events from offset
    pub fn read(&self, from_offset: u64, limit: usize) -> Vec<PartitionEvent> {
        self.buffer
            .iter()
            .filter(|evt| evt.offset >= from_offset)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get partition statistics
    pub fn stats(&self) -> PartitionStats {
        PartitionStats {
            partition_id: self.id,
            topic: self.topic.clone(),
            message_count: self.buffer.len() as u64,
            total_bytes: self.total_bytes,
            min_offset: self.min_offset,
            max_offset: if self.next_offset > 0 {
                self.next_offset - 1
            } else {
                0
            },
            last_compaction: self.last_compaction.elapsed().as_secs(),
        }
    }

    /// Apply retention policy and compact old messages
    pub fn compact(&mut self) -> CompactionResult {
        let initial_count = self.buffer.len();
        let initial_bytes = self.total_bytes;

        // Clone the retention policy to avoid borrow checker issues
        let retention = self.config.retention.clone();

        match retention {
            RetentionPolicy::Time { retention_secs } => {
                let cutoff = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .saturating_sub(retention_secs);

                self.compact_by_time(cutoff);
            }
            RetentionPolicy::Size { max_bytes } => {
                self.compact_by_size(max_bytes);
            }
            RetentionPolicy::Messages { max_messages } => {
                self.compact_by_count(max_messages);
            }
            RetentionPolicy::Combined {
                retention_secs,
                max_bytes,
                max_messages,
            } => {
                if let Some(secs) = retention_secs {
                    let cutoff = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        .saturating_sub(secs);
                    self.compact_by_time(cutoff);
                }
                if let Some(bytes) = max_bytes {
                    self.compact_by_size(bytes);
                }
                if let Some(count) = max_messages {
                    self.compact_by_count(count);
                }
            }
            RetentionPolicy::Infinite => {
                // No compaction
            }
        }

        self.last_compaction = Instant::now();

        CompactionResult {
            messages_removed: initial_count - self.buffer.len(),
            bytes_freed: initial_bytes - self.total_bytes,
        }
    }

    /// Maybe compact if enough time has passed
    fn maybe_compact(&mut self) {
        if self.last_compaction.elapsed() > Duration::from_secs(self.config.flush_interval_secs) {
            self.compact();
        }
    }

    /// Compact by time
    fn compact_by_time(&mut self, cutoff_timestamp: u64) {
        let initial_len = self.buffer.len();
        let mut bytes_removed = 0u64;

        self.buffer.retain(|evt| {
            let keep = evt.timestamp > cutoff_timestamp;
            if !keep {
                bytes_removed += evt.size_bytes;
            }
            keep
        });

        self.total_bytes = self.total_bytes.saturating_sub(bytes_removed);

        if initial_len != self.buffer.len() {
            if let Some(first) = self.buffer.front() {
                self.min_offset = first.offset;
            }
        }
    }

    /// Compact by size
    fn compact_by_size(&mut self, max_bytes: u64) {
        while self.total_bytes > max_bytes && !self.buffer.is_empty() {
            if let Some(evt) = self.buffer.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(evt.size_bytes);
                self.min_offset = evt.offset + 1;
            }
        }
    }

    /// Compact by message count
    fn compact_by_count(&mut self, max_messages: u64) {
        while self.buffer.len() as u64 > max_messages && !self.buffer.is_empty() {
            if let Some(evt) = self.buffer.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(evt.size_bytes);
                self.min_offset = evt.offset + 1;
            }
        }
    }

    /// Get all events (for snapshot/replication)
    pub fn get_all_events(&self) -> Vec<PartitionEvent> {
        self.buffer.iter().cloned().collect()
    }
}

/// Partition statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionStats {
    pub partition_id: usize,
    pub topic: String,
    pub message_count: u64,
    pub total_bytes: u64,
    pub min_offset: u64,
    pub max_offset: u64,
    pub last_compaction: u64,
}

/// Compaction result
#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub messages_removed: usize,
    pub bytes_freed: u64,
}

/// Partitioned topic (like Kafka topic)
pub struct PartitionedTopic {
    name: String,
    partitions: Vec<Partition>,
    config: PartitionConfig,
}

impl PartitionedTopic {
    pub fn new(name: String, config: PartitionConfig) -> Self {
        let mut partitions = Vec::with_capacity(config.num_partitions);
        for i in 0..config.num_partitions {
            partitions.push(Partition::new(i, name.clone(), config.clone()));
        }

        Self {
            name,
            partitions,
            config,
        }
    }

    /// Select partition for an event (using key hash or round-robin)
    pub fn select_partition(&self, key: &Option<Vec<u8>>) -> usize {
        match key {
            Some(k) => {
                // Hash-based partitioning (like Kafka)
                let hash = crc32fast::hash(k);
                (hash as usize) % self.partitions.len()
            }
            None => {
                // Round-robin based on current sizes
                self.partitions
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, p)| p.buffer.len())
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
        }
    }

    /// Publish event to topic
    pub fn publish(&mut self, event: PartitionEvent) -> (usize, u64) {
        let partition_id = self.select_partition(&event.key);
        let offset = self.partitions[partition_id].append(event);
        (partition_id, offset)
    }

    /// Consume from specific partition
    pub fn consume_partition(
        &self,
        partition_id: usize,
        from_offset: u64,
        limit: usize,
    ) -> Result<Vec<PartitionEvent>, String> {
        self.partitions
            .get(partition_id)
            .map(|p| p.read(from_offset, limit))
            .ok_or_else(|| format!("Partition {} not found", partition_id))
    }

    /// Consume from all partitions (consumer group style)
    pub fn consume_all(&self, from_offset: u64, limit: usize) -> Vec<PartitionEvent> {
        let mut all_events = Vec::new();

        for partition in &self.partitions {
            let events = partition.read(from_offset, limit);
            all_events.extend(events);
        }

        // Sort by timestamp for ordered delivery
        all_events.sort_by_key(|e| e.timestamp);
        all_events.truncate(limit);
        all_events
    }

    /// Get statistics for all partitions
    pub fn stats(&self) -> Vec<PartitionStats> {
        self.partitions.iter().map(|p| p.stats()).collect()
    }

    /// Compact all partitions
    pub fn compact_all(&mut self) -> Vec<(usize, CompactionResult)> {
        self.partitions
            .iter_mut()
            .enumerate()
            .map(|(id, p)| (id, p.compact()))
            .collect()
    }

    /// Get partition count
    pub fn partition_count(&self) -> usize {
        self.partitions.len()
    }

    /// Get topic name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get all events from all partitions
    pub fn get_all_events(&self) -> HashMap<usize, Vec<PartitionEvent>> {
        self.partitions
            .iter()
            .enumerate()
            .map(|(id, p)| (id, p.get_all_events()))
            .collect()
    }
}

/// Partition manager for multiple topics
#[derive(Clone)]
pub struct PartitionManager {
    topics: Arc<RwLock<HashMap<String, PartitionedTopic>>>,
    default_config: PartitionConfig,
}

impl PartitionManager {
    pub fn new(default_config: PartitionConfig) -> Self {
        Self {
            topics: Arc::new(RwLock::new(HashMap::new())),
            default_config,
        }
    }

    /// Create a new topic with partitions
    pub async fn create_topic(
        &self,
        topic_name: &str,
        config: Option<PartitionConfig>,
    ) -> Result<(), String> {
        let mut topics = self.topics.write();

        if topics.contains_key(topic_name) {
            return Err(format!("Topic '{}' already exists", topic_name));
        }

        let topic_config = config.unwrap_or_else(|| self.default_config.clone());
        topics.insert(
            topic_name.to_string(),
            PartitionedTopic::new(topic_name.to_string(), topic_config),
        );

        Ok(())
    }

    /// Publish event to topic
    pub async fn publish(
        &self,
        topic: &str,
        event_type: &str,
        key: Option<Vec<u8>>,
        data: Vec<u8>,
    ) -> Result<(usize, u64), String> {
        let mut topics = self.topics.write();

        let topic_obj = topics
            .get_mut(topic)
            .ok_or_else(|| format!("Topic '{}' not found", topic))?;

        let event = PartitionEvent::new(topic.to_string(), event_type.to_string(), key, data);
        Ok(topic_obj.publish(event))
    }

    /// Consume from specific partition
    pub async fn consume_partition(
        &self,
        topic: &str,
        partition_id: usize,
        from_offset: u64,
        limit: usize,
    ) -> Result<Vec<PartitionEvent>, String> {
        let topics = self.topics.read();

        topics
            .get(topic)
            .ok_or_else(|| format!("Topic '{}' not found", topic))?
            .consume_partition(partition_id, from_offset, limit)
    }

    /// Consume from all partitions
    pub async fn consume_all(
        &self,
        topic: &str,
        from_offset: u64,
        limit: usize,
    ) -> Result<Vec<PartitionEvent>, String> {
        let topics = self.topics.read();

        topics
            .get(topic)
            .ok_or_else(|| format!("Topic '{}' not found", topic))
            .map(|t| t.consume_all(from_offset, limit))
    }

    /// Get topic statistics
    pub async fn topic_stats(&self, topic: &str) -> Result<Vec<PartitionStats>, String> {
        let topics = self.topics.read();

        topics
            .get(topic)
            .ok_or_else(|| format!("Topic '{}' not found", topic))
            .map(|t| t.stats())
    }

    /// List all topics
    pub async fn list_topics(&self) -> Vec<String> {
        let topics = self.topics.read();
        topics.keys().cloned().collect()
    }

    /// Delete a topic
    pub async fn delete_topic(&self, topic: &str) -> Result<(), String> {
        let mut topics = self.topics.write();

        if topics.remove(topic).is_some() {
            Ok(())
        } else {
            Err(format!("Topic '{}' not found", topic))
        }
    }

    /// Compact all topics
    pub async fn compact_all(&self) -> HashMap<String, Vec<(usize, CompactionResult)>> {
        let mut topics = self.topics.write();
        let mut results = HashMap::new();

        for (name, topic) in topics.iter_mut() {
            results.insert(name.clone(), topic.compact_all());
        }

        results
    }

    /// Start background compaction task
    pub fn start_compaction_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Every minute

            loop {
                interval.tick().await;
                let _ = self.compact_all().await;
            }
        });
    }

    /// Get all events from all topics (for snapshot)
    pub async fn get_all_events(&self) -> HashMap<String, HashMap<usize, Vec<PartitionEvent>>> {
        let topics = self.topics.read();
        let mut all_events = HashMap::new();

        for (name, topic) in topics.iter() {
            all_events.insert(name.clone(), topic.get_all_events());
        }

        all_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_partition_creation() {
        let config = PartitionConfig {
            num_partitions: 5,
            ..Default::default()
        };

        let manager = PartitionManager::new(config);
        manager.create_topic("test-topic", None).await.unwrap();

        let topics = manager.list_topics().await;
        assert_eq!(topics.len(), 1);
        assert!(topics.contains(&"test-topic".to_string()));
    }

    #[tokio::test]
    async fn test_partition_publish_consume() {
        let manager = PartitionManager::new(PartitionConfig::default());
        manager.create_topic("events", None).await.unwrap();

        // Publish with key
        let (partition_id, offset) = manager
            .publish("events", "update", Some(b"user123".to_vec()), b"data".to_vec())
            .await
            .unwrap();

        assert_eq!(offset, 0);

        // Consume from partition
        let events = manager
            .consume_partition("events", partition_id, 0, 10)
            .await
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, b"data");
    }

    #[tokio::test]
    async fn test_partition_key_based_routing() {
        let config = PartitionConfig {
            num_partitions: 3,
            ..Default::default()
        };

        let manager = PartitionManager::new(config);
        manager.create_topic("orders", None).await.unwrap();

        // Same key should go to same partition
        let (p1, _) = manager
            .publish("orders", "create", Some(b"key1".to_vec()), b"data1".to_vec())
            .await
            .unwrap();

        let (p2, _) = manager
            .publish("orders", "create", Some(b"key1".to_vec()), b"data2".to_vec())
            .await
            .unwrap();

        assert_eq!(p1, p2);
    }

    #[tokio::test]
    async fn test_retention_by_time() {
        let config = PartitionConfig {
            num_partitions: 1,
            retention: RetentionPolicy::Time { retention_secs: 1 },
            ..Default::default()
        };

        let manager = PartitionManager::new(config);
        manager.create_topic("temp", None).await.unwrap();

        // Publish events
        manager
            .publish("temp", "event", None, b"old".to_vec())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_secs(2)).await;

        // Trigger compaction
        manager.compact_all().await;

        // Events should be removed
        let stats = manager.topic_stats("temp").await.unwrap();
        assert_eq!(stats[0].message_count, 0);
    }

    #[tokio::test]
    async fn test_retention_by_size() {
        let config = PartitionConfig {
            num_partitions: 1,
            retention: RetentionPolicy::Size { max_bytes: 100 },
            ..Default::default()
        };

        let manager = PartitionManager::new(config);
        manager.create_topic("limited", None).await.unwrap();

        // Publish large messages
        for i in 0..10 {
            manager
                .publish("limited", "big", None, vec![i; 50])
                .await
                .unwrap();
        }

        // Manually compact
        manager.compact_all().await;

        let stats = manager.topic_stats("limited").await.unwrap();
        assert!(stats[0].total_bytes <= 100);
    }

    #[tokio::test]
    async fn test_retention_by_count() {
        let config = PartitionConfig {
            num_partitions: 1,
            retention: RetentionPolicy::Messages { max_messages: 5 },
            ..Default::default()
        };

        let manager = PartitionManager::new(config);
        manager.create_topic("counted", None).await.unwrap();

        // Publish 10 messages
        for i in 0..10 {
            manager
                .publish("counted", "msg", None, vec![i])
                .await
                .unwrap();
        }

        // Compact
        manager.compact_all().await;

        let stats = manager.topic_stats("counted").await.unwrap();
        assert_eq!(stats[0].message_count, 5);
    }

    #[tokio::test]
    async fn test_combined_retention_policy() {
        let config = PartitionConfig {
            num_partitions: 1,
            retention: RetentionPolicy::Combined {
                retention_secs: Some(3600),
                max_bytes: Some(1000),
                max_messages: Some(10),
            },
            ..Default::default()
        };

        let manager = PartitionManager::new(config);
        manager.create_topic("combined", None).await.unwrap();

        // Publish 20 messages
        for i in 0..20 {
            manager
                .publish("combined", "event", None, vec![i; 10])
                .await
                .unwrap();
        }

        manager.compact_all().await;

        let stats = manager.topic_stats("combined").await.unwrap();
        // Should be limited by max_messages=10
        assert_eq!(stats[0].message_count, 10);
    }

    #[tokio::test]
    async fn test_consume_all_partitions() {
        let config = PartitionConfig {
            num_partitions: 3,
            ..Default::default()
        };

        let manager = PartitionManager::new(config);
        manager.create_topic("broadcast", None).await.unwrap();

        // Publish to different partitions
        for i in 0..9 {
            manager
                .publish(
                    "broadcast",
                    "event",
                    Some(format!("key{}", i).into_bytes()),
                    vec![i],
                )
                .await
                .unwrap();
        }

        // Consume all
        let events = manager.consume_all("broadcast", 0, 100).await.unwrap();
        assert_eq!(events.len(), 9);
    }
}

