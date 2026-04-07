# Rust SDK Documentation

## Overview

The Synap Rust SDK provides a type-safe, async/await client built on Tokio for native Rust applications.

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
synap-client = "0.1"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Quick Start

```rust
use synap_client::{SynapClient, ClientConfig};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SynapClient::new(ClientConfig {
        url: "http://localhost:15500".to_string(),
        api_key: Some("synap_your_api_key".to_string()),
        ..Default::default()
    })?;
    
    // Key-Value operations
    client.kv_set("user:1", json!({"name": "Alice"}), Some(3600)).await?;
    let value = client.kv_get("user:1").await?;
    
    // Queue operations
    client.queue_publish("tasks", json!({"type": "send_email"}), None).await?;
    let message = client.queue_consume("tasks", None).await?;
    client.queue_ack("tasks", &message.message_id).await?;
    
    Ok(())
}
```

## Client Configuration

```rust
use synap_client::{SynapClient, ClientConfig};

pub struct ClientConfig {
    pub url: String,
    pub api_key: Option<String>,
    pub timeout: Duration,           // Default: 30s
    pub retries: u32,                // Default: 3
    pub pool_size: usize,            // Default: 10
    pub keep_alive: bool,            // Default: true
    pub compression: bool,           // Default: false
    pub format: SerializationFormat, // json or msgpack
}

let client = SynapClient::new(ClientConfig {
    url: "http://synap.example.com:15500".to_string(),
    api_key: Some(std::env::var("SYNAP_API_KEY")?),
    timeout: Duration::from_secs(60),
    retries: 5,
    pool_size: 20,
    compression: true,
    format: SerializationFormat::Json,
})?;
```

## Key-Value API

### SET - Store Value

```rust
async fn kv_set<V: Serialize>(
    &self,
    key: &str,
    value: V,
    ttl: Option<u64>,
) -> Result<SetResult>

pub struct SetResult {
    pub key: String,
    pub success: bool,
    pub previous: Option<serde_json::Value>,
}
```

**Example**:
```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct User {
    name: String,
    email: String,
    age: u32,
}

let user = User {
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    age: 30,
};

// Store with 1 hour TTL
client.kv_set("user:1001", &user, Some(3600)).await?;

// Store without TTL
client.kv_set("config:app", &config, None).await?;
```

### GET - Retrieve Value

```rust
async fn kv_get<V: DeserializeOwned>(
    &self,
    key: &str,
) -> Result<GetResult<V>>

pub struct GetResult<V> {
    pub found: bool,
    pub value: Option<V>,
    pub ttl: Option<u64>,
}
```

**Example**:
```rust
let result = client.kv_get::<User>("user:1001").await?;

if let Some(user) = result.value {
    tracing::info!("User: {} ({})", user.name, user.email);
    if let Some(ttl) = result.ttl {
        tracing::info!("Expires in {} seconds", ttl);
    }
} else {
    tracing::info!("User not found");
}
```

### DEL - Delete Keys

```rust
async fn kv_del(&self, keys: &[&str]) -> Result<usize>
```

**Example**:
```rust
// Delete single key
client.kv_del(&["user:1001"]).await?;

// Delete multiple keys
let deleted = client.kv_del(&[
    "user:1001",
    "user:1002",
    "user:1003"
]).await?;

tracing::info!("Deleted {} keys", deleted);
```

### INCR/DECR - Atomic Increment

```rust
async fn kv_incr(&self, key: &str, amount: i64) -> Result<i64>
async fn kv_decr(&self, key: &str, amount: i64) -> Result<i64>
```

**Example**:
```rust
// Increment page views
let views = client.kv_incr("article:123:views", 1).await?;

// Decrement inventory
let remaining = client.kv_decr("inventory:item:42", 1).await?;
if remaining <= 0 {
    tracing::info!("Out of stock!");
}
```

### SCAN - Scan Keys

```rust
async fn kv_scan(
    &self,
    prefix: Option<&str>,
    cursor: Option<&str>,
    count: Option<usize>,
) -> Result<ScanResult>

pub struct ScanResult {
    pub keys: Vec<String>,
    pub cursor: Option<String>,
    pub has_more: bool,
}
```

**Example**:
```rust
let mut all_keys = Vec::new();
let mut cursor = None;

loop {
    let result = client.kv_scan(
        Some("user:"),
        cursor.as_deref(),
        Some(100)
    ).await?;
    
    all_keys.extend(result.keys);
    
    if !result.has_more {
        break;
    }
    
    cursor = result.cursor;
}

tracing::info!("Found {} user keys", all_keys.len());
```

## Queue API

### PUBLISH - Add Message

```rust
async fn queue_publish<M: Serialize>(
    &self,
    queue: &str,
    message: M,
    options: Option<PublishOptions>,
) -> Result<PublishResult>

pub struct PublishOptions {
    pub priority: Option<u8>,  // 0-9
    pub headers: Option<HashMap<String, String>>,
}

pub struct PublishResult {
    pub message_id: String,
    pub position: usize,
}
```

**Example**:
```rust
#[derive(Serialize)]
struct EmailTask {
    to: String,
    subject: String,
    body: String,
}

let task = EmailTask {
    to: "user@example.com".to_string(),
    subject: "Welcome!".to_string(),
    body: "Thanks for signing up".to_string(),
};

let result = client.queue_publish(
    "email-tasks",
    &task,
    Some(PublishOptions {
        priority: Some(8),
        headers: Some(HashMap::from([
            ("source".to_string(), "signup-flow".to_string())
        ])),
    })
).await?;

tracing::info!("Published message {} at position {}", 
         result.message_id, result.position);
```

### CONSUME - Get Message

```rust
async fn queue_consume<M: DeserializeOwned>(
    &self,
    queue: &str,
    options: Option<ConsumeOptions>,
) -> Result<Option<QueueMessage<M>>>

pub struct ConsumeOptions {
    pub timeout: Option<u64>,      // Seconds
    pub ack_deadline: Option<u64>, // Seconds
}

pub struct QueueMessage<M> {
    pub message_id: String,
    pub message: M,
    pub priority: u8,
    pub retry_count: u32,
    pub headers: HashMap<String, String>,
}
```

**Example**:
```rust
#[derive(Deserialize)]
struct EmailTask {
    to: String,
    subject: String,
}

// Wait up to 30 seconds for message
let msg = client.queue_consume::<EmailTask>(
    "email-tasks",
    Some(ConsumeOptions {
        timeout: Some(30),
        ack_deadline: Some(300),
    })
).await?;

if let Some(msg) = msg {
    // Process task
    match send_email(&msg.message).await {
        Ok(_) => {
            // Acknowledge success
            client.queue_ack("email-tasks", &msg.message_id).await?;
        }
        Err(e) => {
            // Negative acknowledge (requeue)
            client.queue_nack("email-tasks", &msg.message_id, true).await?;
        }
    }
}
```

### ACK/NACK - Acknowledge Message

```rust
async fn queue_ack(&self, queue: &str, message_id: &str) -> Result<()>

async fn queue_nack(
    &self,
    queue: &str,
    message_id: &str,
    requeue: bool,
) -> Result<NackResult>

pub struct NackResult {
    pub success: bool,
    pub action: String,  // "requeued" or "dead_lettered"
}
```

## Event Stream API

### PUBLISH - Publish Event

```rust
async fn stream_publish<D: Serialize>(
    &self,
    room: &str,
    event_type: &str,
    data: D,
    metadata: Option<HashMap<String, String>>,
) -> Result<PublishEventResult>

pub struct PublishEventResult {
    pub event_id: String,
    pub offset: u64,
    pub subscribers_notified: usize,
}
```

**Example**:
```rust
#[derive(Serialize)]
struct ChatMessage {
    user: String,
    text: String,
    timestamp: String,
}

let msg = ChatMessage {
    user: "alice".to_string(),
    text: "Hello!".to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
};

let result = client.stream_publish(
    "chat-room-1",
    "message",
    &msg,
    None
).await?;

tracing::info!("Event {} sent to {} subscribers", 
         result.offset, result.subscribers_notified);
```

### SUBSCRIBE - Subscribe to Room

```rust
async fn stream_subscribe<F, Fut>(
    &self,
    room: &str,
    callback: F,
    options: Option<SubscribeOptions>,
) -> Result<Subscription>
where
    F: Fn(StreamEvent) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,

pub struct SubscribeOptions {
    pub from_offset: Option<i64>,  // Can be negative for "last N"
    pub replay: bool,
}

pub struct StreamEvent {
    pub event_id: String,
    pub offset: u64,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: u64,
}

pub struct Subscription {
    // Methods
    pub async fn unsubscribe(&self) -> Result<()>;
}
```

**Example**:
```rust
let subscription = client.stream_subscribe(
    "chat-room-1",
    |event| async move {
        if event.event_type == "message" {
            tracing::info!("Message at offset {}: {:?}", event.offset, event.data);
        }
    },
    Some(SubscribeOptions {
        from_offset: Some(-50),  // Last 50 events
        replay: true,
    })
).await?;

// Later: unsubscribe
subscription.unsubscribe().await?;
```

### HISTORY - Get Event History

```rust
async fn stream_history(
    &self,
    room: &str,
    from_offset: Option<u64>,
    to_offset: Option<u64>,
    limit: Option<usize>,
) -> Result<HistoryResult>

pub struct HistoryResult {
    pub events: Vec<StreamEvent>,
    pub oldest_offset: u64,
    pub newest_offset: u64,
}
```

## Pub/Sub API

### PUBLISH - Publish Message

```rust
async fn pubsub_publish<M: Serialize>(
    &self,
    topic: &str,
    message: M,
    metadata: Option<HashMap<String, String>>,
) -> Result<PublishResult>

pub struct PublishResult {
    pub message_id: String,
    pub topic: String,
    pub subscribers_matched: usize,
}
```

### SUBSCRIBE - Subscribe to Topics

```rust
async fn pubsub_subscribe<F, Fut>(
    &self,
    topics: &[&str],
    callback: F,
) -> Result<Subscription>
where
    F: Fn(String, serde_json::Value) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
```

**Example**:
```rust
let subscription = client.pubsub_subscribe(
    &["notifications.email.*", "events.user.#"],
    |topic, message| async move {
        tracing::info!("[{}] {:?}", topic, message);
    }
).await?;

// Publish message
client.pubsub_publish(
    "notifications.email.user",
    &json!({
        "to": "alice@example.com",
        "subject": "Welcome!"
    }),
    None
).await?;
```

## Error Handling

### Error Types

```rust
use synap_client::error::SynapError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SynapError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    
    #[error("Queue not found: {0}")]
    QueueNotFound(String),
    
    #[error("Room not found: {0}")]
    RoomNotFound(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded { retry_after: u64 },
    
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}
```

### Error Handling Example

```rust
use synap_client::error::SynapError;

match client.kv_get::<String>("nonexistent-key").await {
    Ok(result) => {
        if result.found {
            tracing::info!("Value: {:?}", result.value);
        } else {
            tracing::info!("Key not found");
        }
    }
    Err(SynapError::KeyNotFound(key)) => {
        tracing::info!("Key {} doesn't exist", key);
    }
    Err(SynapError::RateLimitExceeded { retry_after }) => {
        tracing::info!("Rate limited, retry after {} seconds", retry_after);
        tokio::time::sleep(Duration::from_secs(retry_after)).await;
    }
    Err(e) => {
        etracing::info!("Error: {}", e);
    }
}
```

## Type Safety

### Strongly Typed Operations

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
struct User {
    name: String,
    email: String,
    age: u32,
}

// Type-safe SET
client.kv_set("user:1", &user, None).await?;

// Type-safe GET with automatic deserialization
let result = client.kv_get::<User>("user:1").await?;

if let Some(user) = result.value {
    tracing::info!("User: {} ({})", user.name, user.email);
}
```

### Generic Queue Messages

```rust
#[derive(Serialize, Deserialize)]
enum TaskType {
    SendEmail { to: String, subject: String },
    ProcessVideo { video_id: String },
    GenerateThumbnail { image_id: String },
}

// Publish typed message
client.queue_publish("tasks", &TaskType::SendEmail {
    to: "user@example.com".to_string(),
    subject: "Welcome!".to_string(),
}, None).await?;

// Consume with type safety
if let Some(msg) = client.queue_consume::<TaskType>("tasks", None).await? {
    match msg.message {
        TaskType::SendEmail { to, subject } => {
            send_email(&to, &subject).await?;
        }
        TaskType::ProcessVideo { video_id } => {
            process_video(&video_id).await?;
        }
        TaskType::GenerateThumbnail { image_id } => {
            generate_thumbnail(&image_id).await?;
        }
    }
    
    client.queue_ack("tasks", &msg.message_id).await?;
}
```

## Advanced Features

### Connection Pooling

```rust
pub struct ConnectionPool {
    connections: Vec<Connection>,
    available: mpsc::Sender<Connection>,
    config: PoolConfig,
}

impl SynapClient {
    async fn acquire_connection(&self) -> Result<Connection> {
        // Managed automatically by client
    }
}
```

### Retry Logic

```rust
use synap_client::retry::{RetryPolicy, ExponentialBackoff};

let client = SynapClient::builder()
    .url("http://localhost:15500")
    .retry_policy(ExponentialBackoff {
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(30),
        multiplier: 2.0,
        max_retries: 5,
    })
    .build()?;
```

### Batch Operations

```rust
use synap_client::BatchCommand;

let commands = vec![
    BatchCommand::KvSet {
        key: "user:1".to_string(),
        value: json!({"name": "Alice"}),
        ttl: None,
    },
    BatchCommand::KvSet {
        key: "user:2".to_string(),
        value: json!({"name": "Bob"}),
        ttl: None,
    },
    BatchCommand::KvGet {
        key: "user:1".to_string(),
    },
];

let results = client.batch(commands).await?;

for result in results {
    match result {
        Ok(payload) => tracing::info!("Success: {:?}", payload),
        Err(e) => tracing::info!("Error: {}", e),
    }
}
```

## Examples

### Session Manager

```rust
use synap_client::SynapClient;
use uuid::Uuid;

pub struct SessionManager {
    client: SynapClient,
    ttl: u64,
}

impl SessionManager {
    pub fn new(client: SynapClient, ttl: u64) -> Self {
        Self { client, ttl }
    }
    
    pub async fn create_session(&self, user_id: u64) -> Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let session_data = json!({
            "user_id": user_id,
            "created_at": chrono::Utc::now().timestamp(),
        });
        
        self.client.kv_set(
            &format!("session:{}", session_id),
            &session_data,
            Some(self.ttl)
        ).await?;
        
        Ok(session_id)
    }
    
    pub async fn get_session(&self, session_id: &str) -> Result<Option<serde_json::Value>> {
        let result = self.client.kv_get::<serde_json::Value>(
            &format!("session:{}", session_id)
        ).await?;
        
        Ok(result.value)
    }
    
    pub async fn destroy_session(&self, session_id: &str) -> Result<()> {
        self.client.kv_del(&[&format!("session:{}", session_id)]).await?;
        Ok(())
    }
}
```

### Task Queue Worker

```rust
use synap_client::{SynapClient, QueueMessage};
use tokio::signal;

pub struct TaskWorker {
    client: SynapClient,
    queue_name: String,
    running: Arc<AtomicBool>,
}

impl TaskWorker {
    pub fn new(client: SynapClient, queue_name: String) -> Self {
        Self {
            client,
            queue_name,
            running: Arc::new(AtomicBool::new(false)),
        }
    }
    
    pub async fn start(&self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        
        while self.running.load(Ordering::SeqCst) {
            let msg = self.client.queue_consume::<serde_json::Value>(
                &self.queue_name,
                Some(ConsumeOptions {
                    timeout: Some(30),
                    ack_deadline: Some(300),
                })
            ).await?;
            
            if let Some(msg) = msg {
                match self.process_task(&msg).await {
                    Ok(_) => {
                        self.client.queue_ack(&self.queue_name, &msg.message_id).await?;
                    }
                    Err(e) => {
                        etracing::info!("Task processing failed: {}", e);
                        self.client.queue_nack(&self.queue_name, &msg.message_id, true).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
    
    async fn process_task(&self, msg: &QueueMessage<serde_json::Value>) -> Result<()> {
        // Task processing logic
        tracing::info!("Processing: {:?}", msg.message);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = SynapClient::connect("http://localhost:15500").await?;
    let worker = TaskWorker::new(client, "tasks".to_string());
    
    // Graceful shutdown on Ctrl+C
    let running = worker.running.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.unwrap();
        running.store(false, Ordering::SeqCst);
    });
    
    worker.start().await?;
    Ok(())
}
```

### Event Stream Subscriber

```rust
use synap_client::{SynapClient, StreamEvent};

pub async fn subscribe_to_events(client: &SynapClient, room: &str) -> Result<()> {
    let subscription = client.stream_subscribe(
        room,
        |event: StreamEvent| async move {
            match event.event_type.as_str() {
                "message" => handle_message(event.data).await,
                "join" => handle_join(event.data).await,
                "leave" => handle_leave(event.data).await,
                _ => tracing::info!("Unknown event type: {}", event.event_type),
            }
        },
        Some(SubscribeOptions {
            from_offset: Some(-100),
            replay: true,
        })
    ).await?;
    
    // Keep subscription alive
    tokio::signal::ctrl_c().await?;
    subscription.unsubscribe().await?;
    
    Ok(())
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use synap_client::testing::MockClient;
    
    #[tokio::test]
    async fn test_kv_operations() {
        let client = MockClient::new();
        
        // Mock GET response
        client.expect_kv_get()
            .with("user:1")
            .returning(|_| Ok(GetResult {
                found: true,
                value: Some(json!({"name": "Alice"})),
                ttl: Some(3600),
            }));
        
        let result = client.kv_get::<serde_json::Value>("user:1").await.unwrap();
        assert!(result.found);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_integration() -> Result<()> {
    let client = SynapClient::connect("http://localhost:15500").await?;
    
    // Test set and get
    client.kv_set("test-key", &"test-value", None).await?;
    let result = client.kv_get::<String>("test-key").await?;
    
    assert!(result.found);
    assert_eq!(result.value.unwrap(), "test-value");
    
    // Cleanup
    client.kv_del(&["test-key"]).await?;
    
    Ok(())
}
```

## See Also

- [REST_API.md](../api/REST_API.md) - Complete API reference
- [TYPESCRIPT.md](TYPESCRIPT.md) - TypeScript SDK
- [PYTHON.md](PYTHON.md) - Python SDK
- [EXAMPLES](../examples/) - Complete examples

