use parking_lot::RwLock;
use radix_trie::{Trie, TrieCommon};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tracing::{debug, warn};
use uuid::Uuid;

use super::SynapError;

/// Unique identifier for a subscriber
pub type SubscriberId = String;

/// Unique identifier for a message
pub type MessageId = String;

/// Message sender channel for WebSocket delivery
pub type MessageSender = mpsc::UnboundedSender<Message>;

/// Pub/Sub Router - manages topic-based publish/subscribe messaging
#[derive(Clone)]
pub struct PubSubRouter {
    /// Exact topic subscriptions (Trie for efficient prefix matching)
    topics: Arc<RwLock<Trie<String, TopicSubscribers>>>,
    
    /// Wildcard subscriptions (separate for efficiency)
    wildcard_subs: Arc<RwLock<Vec<WildcardSubscription>>>,
    
    /// Active WebSocket connections by subscriber_id
    connections: Arc<RwLock<HashMap<SubscriberId, MessageSender>>>,
    
    /// Statistics
    stats: Arc<RwLock<PubSubStats>>,
}

/// Subscribers for a specific topic
#[derive(Clone)]
pub struct TopicSubscribers {
    pub topic: String,
    pub subscribers: HashSet<SubscriberId>,
    pub message_count: Arc<AtomicU64>,
    pub created_at: u64,
}

/// Wildcard subscription pattern
#[derive(Clone)]
pub struct WildcardSubscription {
    pub pattern: String,
    pub subscriber_id: SubscriberId,
    pub compiled_pattern: WildcardMatcher,
}

/// Compiled wildcard pattern for efficient matching
#[derive(Clone, Debug)]
pub struct WildcardMatcher {
    pub segments: Vec<SegmentMatcher>,
}

/// Segment matcher types
#[derive(Clone, Debug, PartialEq)]
pub enum SegmentMatcher {
    Exact(String),    // Literal string
    SingleLevel,      // * wildcard (matches exactly one level)
    MultiLevel,       // # wildcard (matches zero or more levels)
}

/// Pub/Sub statistics
#[derive(Debug, Clone, Serialize)]
pub struct PubSubStats {
    pub total_topics: usize,
    pub total_subscribers: usize,
    pub total_wildcard_subscriptions: usize,
    pub messages_published: u64,
    pub messages_delivered: u64,
}

/// Published message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub topic: String,
    pub payload: serde_json::Value,
    pub metadata: Option<HashMap<String, String>>,
    pub timestamp: u64,
}

/// Subscription result
#[derive(Debug, Serialize)]
pub struct SubscribeResult {
    pub subscriber_id: SubscriberId,
    pub topics: Vec<String>,
    pub subscription_count: usize,
}

/// Publish result
#[derive(Debug, Serialize)]
pub struct PublishResult {
    pub message_id: MessageId,
    pub topic: String,
    pub subscribers_matched: usize,
}

impl PubSubRouter {
    /// Create a new Pub/Sub router
    pub fn new() -> Self {
        Self {
            topics: Arc::new(RwLock::new(Trie::new())),
            wildcard_subs: Arc::new(RwLock::new(Vec::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(PubSubStats {
                total_topics: 0,
                total_subscribers: 0,
                total_wildcard_subscriptions: 0,
                messages_published: 0,
                messages_delivered: 0,
            })),
        }
    }

    /// Register a WebSocket connection for a subscriber
    pub fn register_connection(&self, subscriber_id: String, sender: MessageSender) {
        let mut connections = self.connections.write();
        connections.insert(subscriber_id.clone(), sender);
        debug!("WebSocket connection registered for subscriber: {}", subscriber_id);
    }

    /// Unregister a WebSocket connection
    pub fn unregister_connection(&self, subscriber_id: &str) {
        let mut connections = self.connections.write();
        connections.remove(subscriber_id);
        debug!("WebSocket connection unregistered for subscriber: {}", subscriber_id);
    }

    /// Subscribe to one or more topics (exact or wildcard)
    pub fn subscribe(&self, topics: Vec<String>) -> Result<SubscribeResult, SynapError> {
        let subscriber_id = Uuid::new_v4().to_string();
        let mut subscription_count = 0;

        for topic_pattern in &topics {
            if Self::is_wildcard_pattern(topic_pattern) {
                // Wildcard subscription
                let matcher = Self::compile_pattern(topic_pattern)?;
                let mut wildcards = self.wildcard_subs.write();
                
                wildcards.push(WildcardSubscription {
                    pattern: topic_pattern.clone(),
                    subscriber_id: subscriber_id.clone(),
                    compiled_pattern: matcher,
                });
                
                subscription_count += 1;
                debug!("Subscriber {} added wildcard pattern: {}", subscriber_id, topic_pattern);
            } else {
                // Exact topic subscription
                let mut topics_map = self.topics.write();
                
                if let Some(topic_subs) = topics_map.get_mut(topic_pattern) {
                    topic_subs.subscribers.insert(subscriber_id.clone());
                } else {
                    topics_map.insert(
                        topic_pattern.clone(),
                        TopicSubscribers {
                            topic: topic_pattern.clone(),
                            subscribers: {
                                let mut set = HashSet::new();
                                set.insert(subscriber_id.clone());
                                set
                            },
                            message_count: Arc::new(AtomicU64::new(0)),
                            created_at: Self::current_timestamp(),
                        },
                    );
                }
                
                subscription_count += 1;
                debug!("Subscriber {} added to topic: {}", subscriber_id, topic_pattern);
            }
        }

        // Update stats
        self.update_stats();

        Ok(SubscribeResult {
            subscriber_id,
            topics,
            subscription_count,
        })
    }

    /// Unsubscribe from topics
    pub fn unsubscribe(&self, subscriber_id: &str, topics: Option<Vec<String>>) -> Result<usize, SynapError> {
        let mut unsubscribed = 0;

        if let Some(topic_list) = topics {
            // Unsubscribe from specific topics
            for topic_pattern in &topic_list {
                if Self::is_wildcard_pattern(topic_pattern) {
                    // Remove wildcard subscription
                    let mut wildcards = self.wildcard_subs.write();
                    let before_len = wildcards.len();
                    wildcards.retain(|sub| {
                        !(sub.subscriber_id == subscriber_id && sub.pattern == *topic_pattern)
                    });
                    unsubscribed += before_len - wildcards.len();
                } else {
                    // Remove exact topic subscription
                    let mut topics_map = self.topics.write();
                    if let Some(topic_subs) = topics_map.get_mut(topic_pattern) {
                        if topic_subs.subscribers.remove(subscriber_id) {
                            unsubscribed += 1;
                        }
                    }
                }
            }
        } else {
            // Unsubscribe from all topics
            let mut topics_map = self.topics.write();
            let all_keys: Vec<String> = topics_map.keys().map(|k| k.clone()).collect();
            for key in all_keys {
                if let Some(topic_subs) = topics_map.get_mut(&key) {
                    if topic_subs.subscribers.remove(subscriber_id) {
                        unsubscribed += 1;
                    }
                }
            }

            // Remove all wildcard subscriptions
            let mut wildcards = self.wildcard_subs.write();
            let before_len = wildcards.len();
            wildcards.retain(|sub| sub.subscriber_id != subscriber_id);
            unsubscribed += before_len - wildcards.len();
        }

        // Update stats
        self.update_stats();

        debug!("Subscriber {} unsubscribed from {} topics", subscriber_id, unsubscribed);
        Ok(unsubscribed)
    }

    /// Publish a message to a topic
    pub fn publish(
        &self,
        topic: &str,
        payload: serde_json::Value,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<PublishResult, SynapError> {
        // Validate topic
        if topic.is_empty() {
            return Err(SynapError::InvalidValue("Topic cannot be empty".to_string()));
        }

        // Create message
        let message = Message {
            id: Uuid::new_v4().to_string(),
            topic: topic.to_string(),
            payload,
            metadata,
            timestamp: Self::current_timestamp(),
        };

        // Find all matching subscribers
        let mut subscribers = self.find_exact_subscribers(topic);
        subscribers.extend(self.find_wildcard_subscribers(topic));

        let subscriber_count = subscribers.len();

        // Update message count for exact topic
        {
            let topics_map = self.topics.read();
            if let Some(topic_subs) = topics_map.get(topic) {
                topic_subs.message_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.messages_published += 1;
            stats.messages_delivered += subscriber_count as u64;
        }

        debug!(
            "Published message {} to topic {} ({} subscribers)",
            message.id, topic, subscriber_count
        );

        // Deliver messages to active WebSocket connections
        let delivered = self.deliver_message(&message, &subscribers);

        debug!(
            "Delivered message {} to {}/{} subscribers",
            message.id, delivered, subscriber_count
        );

        Ok(PublishResult {
            message_id: message.id,
            topic: topic.to_string(),
            subscribers_matched: subscriber_count,
        })
    }

    /// Get statistics
    pub fn get_stats(&self) -> PubSubStats {
        self.stats.read().clone()
    }

    /// List all topics
    pub fn list_topics(&self) -> Vec<String> {
        let topics_map = self.topics.read();
        topics_map.keys().map(|k| k.to_string()).collect()
    }

    /// Get topic info
    pub fn get_topic_info(&self, topic: &str) -> Option<TopicInfo> {
        let topics_map = self.topics.read();
        topics_map.get(topic).map(|subs| TopicInfo {
            topic: subs.topic.clone(),
            subscriber_count: subs.subscribers.len(),
            message_count: subs.message_count.load(Ordering::Relaxed),
            created_at: subs.created_at,
        })
    }

    // Private helper methods

    /// Check if pattern contains wildcards
    fn is_wildcard_pattern(pattern: &str) -> bool {
        pattern.contains('*') || pattern.contains('#')
    }

    /// Compile a wildcard pattern into a matcher
    fn compile_pattern(pattern: &str) -> Result<WildcardMatcher, SynapError> {
        let segments: Vec<SegmentMatcher> = pattern
            .split('.')
            .map(|seg| match seg {
                "*" => SegmentMatcher::SingleLevel,
                "#" => SegmentMatcher::MultiLevel,
                s => SegmentMatcher::Exact(s.to_string()),
            })
            .collect();

        // Validate: # can only be at the end
        let multi_level_count = segments.iter().filter(|s| matches!(s, SegmentMatcher::MultiLevel)).count();
        if multi_level_count > 1 {
            return Err(SynapError::InvalidValue(
                "Pattern can only contain one # wildcard".to_string(),
            ));
        }

        if multi_level_count == 1 && !matches!(segments.last(), Some(SegmentMatcher::MultiLevel)) {
            return Err(SynapError::InvalidValue(
                "# wildcard must be at the end of pattern".to_string(),
            ));
        }

        Ok(WildcardMatcher { segments })
    }

    /// Find exact topic subscribers
    fn find_exact_subscribers(&self, topic: &str) -> HashSet<SubscriberId> {
        let topics_map = self.topics.read();
        topics_map
            .get(topic)
            .map(|t| t.subscribers.clone())
            .unwrap_or_default()
    }

    /// Find wildcard subscribers that match the topic
    fn find_wildcard_subscribers(&self, topic: &str) -> HashSet<SubscriberId> {
        let topic_segments: Vec<&str> = topic.split('.').collect();
        let wildcards = self.wildcard_subs.read();

        wildcards
            .iter()
            .filter(|sub| sub.compiled_pattern.matches(&topic_segments))
            .map(|sub| sub.subscriber_id.clone())
            .collect()
    }

    /// Update statistics from current state
    fn update_stats(&self) {
        let topics_map = self.topics.read();
        let wildcards = self.wildcard_subs.read();

        let all_keys: Vec<String> = topics_map.keys().map(|k| k.clone()).collect();
        let total_exact_subscribers: usize = all_keys
            .iter()
            .filter_map(|key| topics_map.get(key))
            .map(|t| t.subscribers.len())
            .sum();

        let mut stats = self.stats.write();
        stats.total_topics = topics_map.len();
        stats.total_subscribers = total_exact_subscribers + wildcards.len();
        stats.total_wildcard_subscriptions = wildcards.len();
    }

    /// Get current Unix timestamp in seconds
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Deliver message to active WebSocket subscribers
    fn deliver_message(&self, message: &Message, subscribers: &HashSet<SubscriberId>) -> usize {
        let connections = self.connections.read();
        let mut delivered = 0;

        for sub_id in subscribers {
            if let Some(sender) = connections.get(sub_id) {
                // Try to send message to WebSocket (non-blocking, unbounded channel)
                if sender.send(message.clone()).is_ok() {
                    delivered += 1;
                } else {
                    warn!("Failed to deliver message to subscriber: {}", sub_id);
                }
            }
        }

        delivered
    }
}

impl WildcardMatcher {
    /// Check if this pattern matches the given topic segments
    pub fn matches(&self, topic_segments: &[&str]) -> bool {
        let mut seg_idx = 0;

        for (pattern_idx, matcher) in self.segments.iter().enumerate() {
            match matcher {
                SegmentMatcher::Exact(s) => {
                    if seg_idx >= topic_segments.len() || topic_segments[seg_idx] != s {
                        return false;
                    }
                    seg_idx += 1;
                }
                SegmentMatcher::SingleLevel => {
                    if seg_idx >= topic_segments.len() {
                        return false;
                    }
                    seg_idx += 1;
                }
                SegmentMatcher::MultiLevel => {
                    // # matches everything remaining
                    // Must be the last segment (validated during compilation)
                    debug_assert!(pattern_idx == self.segments.len() - 1);
                    return true;
                }
            }
        }

        // All pattern segments matched and all topic segments consumed
        seg_idx == topic_segments.len()
    }
}

/// Topic information
#[derive(Debug, Serialize)]
pub struct TopicInfo {
    pub topic: String,
    pub subscriber_count: usize,
    pub message_count: u64,
    pub created_at: u64,
}

impl Default for PubSubRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_subscription() {
        let router = PubSubRouter::new();
        
        let result = router.subscribe(vec!["test.topic".to_string()]).unwrap();
        assert_eq!(result.subscription_count, 1);
        
        let stats = router.get_stats();
        assert_eq!(stats.total_topics, 1);
        assert_eq!(stats.total_subscribers, 1);
    }

    #[test]
    fn test_wildcard_single_level() {
        let router = PubSubRouter::new();
        
        let result = router.subscribe(vec!["notifications.*".to_string()]).unwrap();
        assert_eq!(result.subscription_count, 1);
        
        // Publish to matching topic
        let publish_result = router.publish("notifications.email", serde_json::json!({"test": true}), None).unwrap();
        assert_eq!(publish_result.subscribers_matched, 1);
        
        // Publish to non-matching topic
        let publish_result = router.publish("notifications.email.user", serde_json::json!({"test": true}), None).unwrap();
        assert_eq!(publish_result.subscribers_matched, 0);
    }

    #[test]
    fn test_wildcard_multi_level() {
        let router = PubSubRouter::new();
        
        let result = router.subscribe(vec!["events.user.#".to_string()]).unwrap();
        assert_eq!(result.subscription_count, 1);
        
        // All these should match
        let topics = vec![
            "events.user",
            "events.user.login",
            "events.user.login.success",
        ];
        
        for topic in topics {
            let publish_result = router.publish(topic, serde_json::json!({}), None).unwrap();
            assert_eq!(publish_result.subscribers_matched, 1, "Topic {} should match", topic);
        }
        
        // This should not match
        let publish_result = router.publish("events.admin", serde_json::json!({}), None).unwrap();
        assert_eq!(publish_result.subscribers_matched, 0);
    }

    #[test]
    fn test_pattern_compilation() {
        // Valid patterns
        assert!(PubSubRouter::compile_pattern("test.*").is_ok());
        assert!(PubSubRouter::compile_pattern("test.#").is_ok());
        assert!(PubSubRouter::compile_pattern("*.test.*").is_ok());
        
        // Invalid patterns (# not at end)
        assert!(PubSubRouter::compile_pattern("#.test").is_err());
        assert!(PubSubRouter::compile_pattern("test.#.more").is_err());
        
        // Invalid patterns (multiple #)
        assert!(PubSubRouter::compile_pattern("#.#").is_err());
    }

    #[test]
    fn test_unsubscribe() {
        let router = PubSubRouter::new();
        
        let result = router.subscribe(vec!["topic1".to_string(), "topic2".to_string()]).unwrap();
        let sub_id = result.subscriber_id;
        
        // Unsubscribe from one topic
        let count = router.unsubscribe(&sub_id, Some(vec!["topic1".to_string()])).unwrap();
        assert_eq!(count, 1);
        
        // Verify only one subscription remains
        let publish_result = router.publish("topic1", serde_json::json!({}), None).unwrap();
        assert_eq!(publish_result.subscribers_matched, 0);
        
        let publish_result = router.publish("topic2", serde_json::json!({}), None).unwrap();
        assert_eq!(publish_result.subscribers_matched, 1);
    }

    #[test]
    fn test_wildcard_matcher() {
        // Single level wildcard
        let matcher = WildcardMatcher {
            segments: vec![
                SegmentMatcher::Exact("notifications".to_string()),
                SegmentMatcher::SingleLevel,
            ],
        };
        
        assert!(matcher.matches(&["notifications", "email"]));
        assert!(matcher.matches(&["notifications", "sms"]));
        assert!(!matcher.matches(&["notifications", "email", "user"]));
        assert!(!matcher.matches(&["notifications"]));
        
        // Multi-level wildcard
        let matcher = WildcardMatcher {
            segments: vec![
                SegmentMatcher::Exact("events".to_string()),
                SegmentMatcher::Exact("user".to_string()),
                SegmentMatcher::MultiLevel,
            ],
        };
        
        assert!(matcher.matches(&["events", "user"]));
        assert!(matcher.matches(&["events", "user", "login"]));
        assert!(matcher.matches(&["events", "user", "login", "success"]));
        assert!(!matcher.matches(&["events", "admin"]));
    }
}

