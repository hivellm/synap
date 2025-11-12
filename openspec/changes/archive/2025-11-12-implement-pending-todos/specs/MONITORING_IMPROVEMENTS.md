# Monitoring Improvements Specification

## Overview

This specification covers WebSocket client tracking for monitoring purposes.

## WebSocket Client Tracking

### Current State

- `ClientListManager` exists (`synap-server/src/monitoring/client_list.rs`)
- WebSocket handlers exist for PubSub, Queue, and Stream
- Client tracking is NOT implemented for WebSocket connections
- REST endpoint `client_list()` returns empty array
- StreamableHTTP `client.list` command returns empty array

### Proposed Changes

#### 1. Add Client Tracking to AppState

**File**: `synap-server/src/lib.rs`

Add `ClientListManager` to `AppState`:

```rust
pub struct AppState {
    // ... existing fields ...
    client_list_manager: Arc<ClientListManager>, // NEW
}
```

#### 2. Initialize Client List Manager

**File**: `synap-server/src/main.rs`

Initialize client list manager:

```rust
let client_list_manager = Arc::new(ClientListManager::new());
```

#### 3. Track PubSub WebSocket Connections

**File**: `synap-server/src/server/handlers.rs` (line ~5692)

Add client tracking:

```rust
async fn handle_pubsub_socket(
    socket: WebSocket,
    pubsub_router: Arc<crate::core::PubSubRouter>,
    topics: Vec<String>,
    client_list_manager: Arc<ClientListManager>, // NEW
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    
    // Generate client ID
    let client_id = uuid::Uuid::new_v4().to_string();
    let client_addr = "websocket"; // Could extract from socket if available
    
    // Register client
    let client_info = ClientInfo::new(
        client_id.clone(),
        client_addr.to_string(),
        SystemTime::now(),
    );
    client_list_manager.add(client_info).await;
    
    // ... existing subscription code ...
    
    // Cleanup on disconnect
    let client_list_manager_clone = client_list_manager.clone();
    tokio::spawn(async move {
        // Wait for connection to close
        // ...
        client_list_manager_clone.remove(&client_id).await;
    });
}
```

#### 4. Track Queue WebSocket Connections

**File**: `synap-server/src/server/handlers.rs` (line ~5394)

Add client tracking similar to PubSub:

```rust
async fn handle_queue_socket(
    socket: WebSocket,
    queue_manager: Arc<QueueManager>,
    queue_name: String,
    consumer_id: String,
    client_list_manager: Arc<ClientListManager>, // NEW
) {
    // Similar pattern to PubSub tracking
}
```

#### 5. Track Stream WebSocket Connections

**File**: `synap-server/src/server/handlers.rs`

Add client tracking similar to PubSub:

```rust
async fn handle_stream_socket(
    socket: WebSocket,
    stream_manager: Arc<StreamManager>,
    room: String,
    client_list_manager: Arc<ClientListManager>, // NEW
) {
    // Similar pattern to PubSub tracking
}
```

#### 6. Update REST Client List Handler

**File**: `synap-server/src/server/handlers.rs` (line ~858)

Replace TODO with actual implementation:

```rust
pub async fn client_list(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, SynapError> {
    let clients = state.client_list_manager.list().await;
    
    Ok(Json(serde_json::json!({
        "clients": clients.iter().map(|c| c.to_redis_format()).collect::<Vec<_>>(),
        "count": clients.len()
    })))
}
```

#### 7. Update StreamableHTTP Client List Handler

**File**: `synap-server/src/server/handlers.rs` (line ~6305)

Replace TODO with actual implementation:

```rust
async fn handle_client_list_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let clients = state.client_list_manager.list().await;
    
    Ok(serde_json::json!({
        "clients": clients,
        "count": clients.len()
    }))
}
```

#### 8. Track Client Activity

**File**: `synap-server/src/monitoring/client_list.rs`

Enhance `ClientInfo` to track activity:

```rust
pub struct ClientInfo {
    // ... existing fields ...
    pub last_command: Option<String>, // NEW
    pub last_activity: SystemTime,    // NEW
}

impl ClientInfo {
    pub fn update_activity(&mut self, command: String) {
        self.last_command = Some(command);
        self.last_activity = SystemTime::now();
    }
    
    pub fn idle_time(&self) -> u64 {
        SystemTime::now()
            .duration_since(self.last_activity)
            .unwrap_or_default()
            .as_secs()
    }
}
```

### Testing Requirements

- [ ] Unit test: Client registration
- [ ] Unit test: Client unregistration
- [ ] Unit test: Client activity tracking
- [ ] Integration test: Client list endpoint returns WebSocket clients
- [ ] Test: Client tracking with PubSub WebSocket
- [ ] Test: Client tracking with Queue WebSocket
- [ ] Test: Client tracking with Stream WebSocket
- [ ] Test: Client cleanup on disconnect
- [ ] Test: Multiple concurrent WebSocket connections
- [ ] Test: Client list format matches Redis CLIENT LIST format

### Performance Considerations

- Client tracking adds minimal overhead (~100ns per operation)
- Client list updates are async and non-blocking
- Memory usage: ~200 bytes per client
- Consider periodic cleanup of stale connections

### Future Enhancements

- Track HTTP connections (long-polling)
- Track command history per client
- Track memory usage per client
- Client kill command (CLIENT KILL)
- Client pause/resume (CLIENT PAUSE)

