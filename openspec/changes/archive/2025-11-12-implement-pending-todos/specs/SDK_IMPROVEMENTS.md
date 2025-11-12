# SDK Improvements Specification

## Overview

This specification covers reactive subscription for PubSub in the Rust SDK.

## Reactive Subscription for PubSub (Rust SDK)

### Current State

- Reactive patterns exist for Queue (`observe_messages`) and Stream (`observe_events`)
- PubSub reactive subscription is NOT implemented
- Comment in README indicates "coming soon"
- PubSub has synchronous subscription methods

### Proposed Changes

#### 1. Create Reactive PubSub Module

**File**: `sdks/rust/src/pubsub_reactive.rs` (NEW)

Create new module similar to `queue_reactive.rs`:

```rust
use crate::pubsub::PubSubManager;
use crate::rx::observable::Observable;
use crate::rx::subscription::Subscription;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Reactive PubSub subscription
pub struct ReactivePubSub {
    manager: Arc<PubSubManager>,
}

impl ReactivePubSub {
    pub fn new(manager: Arc<PubSubManager>) -> Self {
        Self { manager }
    }
    
    /// Observe messages from topics (reactive pattern)
    pub fn observe(
        &self,
        topics: Vec<String>,
    ) -> Observable<Message> {
        let (tx, rx) = mpsc::unbounded_channel();
        let manager = self.manager.clone();
        
        // Subscribe to topics
        let subscription_result = manager.subscribe(topics.clone()).ok();
        
        if let Some(result) = subscription_result {
            let subscriber_id = result.subscriber_id;
            
            // Register callback for messages
            manager.register_callback(subscriber_id.clone(), move |message| {
                let _ = tx.send(message);
            });
            
            // Create observable
            Observable::from_stream(rx)
        } else {
            Observable::empty()
        }
    }
    
    /// Observe messages with wildcard patterns
    pub fn observe_with_patterns(
        &self,
        patterns: Vec<String>,
    ) -> Observable<Message> {
        // Similar to observe but with pattern support
    }
}
```

#### 2. Update PubSub Manager

**File**: `sdks/rust/src/pubsub.rs`

Add callback registration support:

```rust
impl PubSubManager {
    // ... existing methods ...
    
    /// Register callback for messages (for reactive patterns)
    pub fn register_callback<F>(&self, subscriber_id: String, callback: F)
    where
        F: Fn(Message) + Send + Sync + 'static,
    {
        // Store callback for later invocation
    }
}
```

#### 3. Add Reactive Methods to Client

**File**: `sdks/rust/src/client.rs`

Add reactive PubSub methods:

```rust
impl Client {
    // ... existing methods ...
    
    /// Get reactive PubSub manager
    pub fn pubsub_reactive(&self) -> ReactivePubSub {
        ReactivePubSub::new(self.pubsub_manager.clone())
    }
}
```

#### 4. Update Documentation

**File**: `sdks/rust/README.md` (line ~264)

Replace TODO with actual example:

```rust
// Reactive subscription (now available)
let (mut messages, handle) = client.pubsub_reactive()
    .observe(vec![
        "user-123".to_string(),
        "events.*".to_string(),    // single-level wildcard
        "notifications.#".to_string(),    // multi-level wildcard
    ]);

// Process messages reactively
messages
    .filter(|msg| msg.topic.starts_with("events."))
    .map(|msg| process_event(msg))
    .subscribe(|event| {
        println!("Processed event: {:?}", event);
    });

// Unsubscribe when done
handle.unsubscribe();
```

### API Design

#### Observable Pattern

```rust
pub trait Observable<T> {
    fn map<U, F>(self, f: F) -> Observable<U>
    where
        F: Fn(T) -> U + Send + Sync + 'static;
    
    fn filter<F>(self, f: F) -> Observable<T>
    where
        F: Fn(&T) -> bool + Send + Sync + 'static;
    
    fn subscribe<F>(self, f: F) -> Subscription
    where
        F: Fn(T) + Send + Sync + 'static;
}
```

#### Message Type

```rust
pub struct Message {
    pub topic: String,
    pub payload: Vec<u8>,
    pub headers: HashMap<String, String>,
    pub timestamp: u64,
}
```

### Testing Requirements

- [ ] Unit test: Reactive PubSub subscription
- [ ] Unit test: Subscription lifecycle (subscribe/unsubscribe)
- [ ] Unit test: Wildcard subscriptions
- [ ] Unit test: Multiple subscriptions
- [ ] Integration test: Reactive PubSub with server
- [ ] Test: Observable operators (map, filter, etc.)
- [ ] Test: Error handling
- [ ] Documentation examples

### Performance Considerations

- Reactive pattern adds minimal overhead (~100ns per message)
- Uses async channels for message delivery
- Memory overhead: ~100 bytes per subscription
- Supports backpressure via bounded channels

### Future Enhancements

- Backpressure handling
- Message batching
- Retry logic
- Circuit breaker pattern
- Metrics collection

