# StreamableHTTP Protocol Specification

## Overview

StreamableHTTP is a lightweight protocol built on top of HTTP/1.1 and HTTP/2 that enables request-response and streaming communication patterns for Synap.

## Design Principles

1. **HTTP-Based**: Uses standard HTTP as transport
2. **Universal**: Works with any HTTP client
3. **Debuggable**: Human-readable JSON messages
4. **Streaming**: Supports chunked transfer encoding
5. **Upgradeable**: Can upgrade to WebSocket

## Protocol Layers

```
┌─────────────────────────────────────┐
│   Application Layer (Commands)      │
├─────────────────────────────────────┤
│   Message Envelope (JSON)           │
├─────────────────────────────────────┤
│   Transfer Encoding (Chunked/WS)    │
├─────────────────────────────────────┤
│   HTTP/1.1 or HTTP/2                │
├─────────────────────────────────────┤
│   TCP                                │
└─────────────────────────────────────┘
```

## Message Envelope Format

### Request Envelope

```json
{
  "type": "request",
  "request_id": "uuid-v4",
  "command": "kv.set",
  "version": "1.0",
  "payload": {
    "key": "user:1",
    "value": "data"
  }
}
```

**Fields**:
- `type` (string): Always "request"
- `request_id` (string): Unique identifier for request/response matching
- `command` (string): Command name (e.g., "kv.set", "queue.publish")
- `version` (string): Protocol version (currently "1.0")
- `payload` (object): Command-specific parameters

### Response Envelope

```json
{
  "type": "response",
  "request_id": "uuid-v4",
  "status": "success",
  "payload": {
    "key": "user:1",
    "success": true
  }
}
```

**Fields**:
- `type` (string): Always "response"
- `request_id` (string): Matches request ID
- `status` (string): "success" or "error"
- `payload` (object): Response data or error details
- `error` (object, optional): Error information if status is "error"

### Error Response

```json
{
  "type": "response",
  "request_id": "uuid-v4",
  "status": "error",
  "error": {
    "code": "KEY_NOT_FOUND",
    "message": "Key 'user:999' not found",
    "details": {
      "key": "user:999"
    }
  }
}
```

## Transport Modes

### 1. Request-Response (HTTP)

Standard HTTP request/response for stateless operations.

**HTTP Request**:
```http
POST /api/v1/command HTTP/1.1
Host: localhost:15500
Content-Type: application/json
Content-Length: 142

{
  "type": "request",
  "request_id": "req-001",
  "command": "kv.get",
  "version": "1.0",
  "payload": {"key": "user:1"}
}
```

**HTTP Response**:
```http
HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 128

{
  "type": "response",
  "request_id": "req-001",
  "status": "success",
  "payload": {
    "found": true,
    "value": "data"
  }
}
```

### 2. Streaming (Chunked Transfer)

For streaming responses (event history, queue consume with wait).

**HTTP Request**:
```http
POST /api/v1/command HTTP/1.1
Host: localhost:15500
Content-Type: application/json

{
  "command": "stream.history",
  "request_id": "req-002",
  "payload": {"room": "chat-1", "from_offset": 0}
}
```

**HTTP Response (Chunked)**:
```http
HTTP/1.1 200 OK
Content-Type: application/json
Transfer-Encoding: chunked

{"offset": 1, "type": "message", "data": {...}}
{"offset": 2, "type": "join", "data": {...}}
{"offset": 3, "type": "message", "data": {...}}
```

Each line is a separate JSON object (newline-delimited JSON).

### 3. WebSocket (Persistent Connection)

For bi-directional real-time communication.

**Upgrade Request**:
```http
GET /api/v1/ws HTTP/1.1
Host: localhost:15500
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13
```

**Upgrade Response**:
```http
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
```

**WebSocket Messages**:
```json
// Client → Server (subscribe)
{
  "type": "request",
  "request_id": "ws-001",
  "command": "stream.subscribe",
  "payload": {"room": "chat-1"}
}

// Server → Client (events)
{"type": "event", "offset": 10, "event_type": "message", "data": {...}}
{"type": "event", "offset": 11, "event_type": "message", "data": {...}}
```

## Command Routing

### URL Structure

**Base URL**: `http://localhost:15500`

**Endpoints**:
- `/api/v1/command` - All commands (POST)
- `/api/v1/ws` - WebSocket upgrade (GET)
- `/health` - Health check (GET)
- `/metrics` - Prometheus metrics (GET)

### Command Namespaces

Commands use dot notation for namespacing:

```
kv.set, kv.get, kv.del         → Key-Value operations
queue.publish, queue.consume    → Queue operations
stream.publish, stream.subscribe → Event stream operations
pubsub.publish, pubsub.subscribe → Pub/Sub operations
admin.stats, admin.config       → Administrative operations
```

## HTTP Status Codes

### Success Codes
- **200 OK**: Successful operation
- **201 Created**: Resource created (queue, room, etc.)
- **204 No Content**: Successful deletion

### Client Error Codes
- **400 Bad Request**: Invalid message format
- **404 Not Found**: Resource not found (key, queue, room)
- **409 Conflict**: Resource already exists
- **422 Unprocessable Entity**: Valid format but invalid data

### Server Error Codes
- **500 Internal Server Error**: Server-side error
- **503 Service Unavailable**: Server overloaded or shutting down
- **507 Insufficient Storage**: Memory limit exceeded

## Content Negotiation

### Request Format

**JSON (Default)**:
```http
Content-Type: application/json
```

**MessagePack (Binary)**:
```http
Content-Type: application/msgpack
```

### Response Format

Matches request Content-Type:
```http
Accept: application/json
Accept: application/msgpack
```

## Streaming Patterns

### Server-Sent Events (SSE)

Alternative to chunked transfer for event streaming:

```http
GET /api/v1/stream/subscribe?room=chat-1 HTTP/1.1
Accept: text/event-stream
```

```http
HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache

event: message
data: {"offset": 10, "type": "message", "data": {...}}

event: message  
data: {"offset": 11, "type": "message", "data": {...}}
```

### Long Polling

For queue consume with wait:

```http
POST /api/v1/command HTTP/1.1

{
  "command": "queue.consume",
  "payload": {
    "queue": "tasks",
    "timeout": 30
  }
}
```

Server holds connection for up to 30 seconds until message available.

## Authentication

### API Key Header

```http
POST /api/v1/command HTTP/1.1
X-API-Key: synap_1234567890abcdef
```

### Bearer Token

```http
POST /api/v1/command HTTP/1.1
Authorization: Bearer eyJhbGciOiJIUzI1NiIs...
```

## Error Response Format

```json
{
  "type": "response",
  "request_id": "req-001",
  "status": "error",
  "error": {
    "code": "QUEUE_NOT_FOUND",
    "message": "Queue 'tasks' does not exist",
    "details": {
      "queue": "tasks",
      "suggestion": "Create queue with queue.create command"
    }
  }
}
```

### Standard Error Codes

| Code | Description | HTTP Status |
|------|-------------|-------------|
| INVALID_REQUEST | Malformed request | 400 |
| INVALID_COMMAND | Unknown command | 400 |
| INVALID_PAYLOAD | Invalid parameters | 422 |
| KEY_NOT_FOUND | Key doesn't exist | 404 |
| QUEUE_NOT_FOUND | Queue doesn't exist | 404 |
| ROOM_NOT_FOUND | Room doesn't exist | 404 |
| QUEUE_FULL | Queue at capacity | 507 |
| MEMORY_LIMIT | Memory limit exceeded | 507 |
| UNAUTHORIZED | Invalid API key | 401 |
| FORBIDDEN | Insufficient permissions | 403 |
| INTERNAL_ERROR | Server error | 500 |

## Request/Response Examples

### Key-Value SET

**Request**:
```http
POST /api/v1/command HTTP/1.1
Content-Type: application/json
X-API-Key: synap_key123

{
  "type": "request",
  "request_id": "req-kv-set-1",
  "command": "kv.set",
  "version": "1.0",
  "payload": {
    "key": "user:1001",
    "value": {"name": "Alice", "email": "alice@example.com"},
    "ttl": 3600
  }
}
```

**Response**:
```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "type": "response",
  "request_id": "req-kv-set-1",
  "status": "success",
  "payload": {
    "key": "user:1001",
    "success": true
  }
}
```

### Queue PUBLISH

**Request**:
```http
POST /api/v1/command HTTP/1.1
Content-Type: application/json

{
  "request_id": "req-q-pub-1",
  "command": "queue.publish",
  "payload": {
    "queue": "tasks",
    "message": {"task": "process_video", "id": "vid_123"},
    "priority": 8
  }
}
```

**Response**:
```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "request_id": "req-q-pub-1",
  "status": "success",
  "payload": {
    "message_id": "msg_abc123",
    "position": 5
  }
}
```

### Event Stream SUBSCRIBE

**Request (WebSocket)**:
```json
{
  "request_id": "ws-sub-1",
  "command": "stream.subscribe",
  "payload": {
    "room": "chat-room-1",
    "from_offset": 0,
    "replay": true
  }
}
```

**Response Stream**:
```json
{"type": "ack", "request_id": "ws-sub-1", "subscribed": true}
{"type": "event", "offset": 1, "event_type": "message", "data": {...}}
{"type": "event", "offset": 2, "event_type": "message", "data": {...}}
...
```

## Performance Considerations

### Connection Pooling

Clients should maintain connection pool for HTTP:

```typescript
const client = new SynapClient({
  url: 'http://localhost:15500',
  poolSize: 10,
  keepAlive: true
});
```

### Request Batching

Batch multiple commands in single HTTP request:

```json
{
  "type": "batch",
  "request_id": "batch-001",
  "commands": [
    {"command": "kv.get", "payload": {"key": "user:1"}},
    {"command": "kv.get", "payload": {"key": "user:2"}},
    {"command": "kv.get", "payload": {"key": "user:3"}}
  ]
}
```

**Batch Response**:
```json
{
  "type": "batch_response",
  "request_id": "batch-001",
  "results": [
    {"status": "success", "payload": {...}},
    {"status": "success", "payload": {...}},
    {"status": "error", "error": {...}}
  ]
}
```

### Compression

Support gzip compression for large payloads:

```http
POST /api/v1/command HTTP/1.1
Content-Encoding: gzip
Accept-Encoding: gzip
```

## Protocol Versioning

### Version Header

```http
X-Synap-Protocol-Version: 1.0
```

### Backward Compatibility

- V1.0: Initial protocol
- V1.1: (Future) Add binary MessagePack support
- V2.0: (Future) Breaking changes require major version bump

## WebSocket Sub-Protocol

### Frame Format

**Text Frames**: JSON messages
**Binary Frames**: MessagePack messages

### Ping/Pong

Server sends ping every 30 seconds:
```json
{"type": "ping", "timestamp": 1697410800}
```

Client responds with pong:
```json
{"type": "pong", "timestamp": 1697410800}
```

### Graceful Disconnect

Client sends close frame:
```json
{"type": "close", "reason": "client_shutdown"}
```

## Security

### TLS Support

Production deployments should use HTTPS/WSS:

```yaml
server:
  tls:
    enabled: true
    cert_path: "/etc/synap/cert.pem"
    key_path: "/etc/synap/key.pem"
```

### Rate Limiting

```http
HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1697410860

{
  "status": "error",
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit of 1000 requests/min exceeded"
  }
}
```

## Implementation Guidelines

### Server-Side (Axum)

```rust
use axum::{
    routing::post,
    extract::Json,
    Router,
};

async fn command_handler(
    Json(envelope): Json<RequestEnvelope>
) -> Json<ResponseEnvelope> {
    let result = match envelope.command.as_str() {
        "kv.set" => handle_kv_set(envelope.payload).await,
        "kv.get" => handle_kv_get(envelope.payload).await,
        _ => Err(SynapError::InvalidCommand),
    };
    
    Json(ResponseEnvelope {
        request_id: envelope.request_id,
        status: if result.is_ok() { "success" } else { "error" },
        payload: result.ok(),
        error: result.err(),
    })
}

let app = Router::new()
    .route("/api/v1/command", post(command_handler));
```

### Client-Side (TypeScript)

```typescript
class SynapClient {
  async sendCommand(command: string, payload: any): Promise<any> {
    const envelope = {
      type: 'request',
      request_id: uuidv4(),
      command,
      version: '1.0',
      payload
    };
    
    const response = await fetch(`${this.baseUrl}/api/v1/command`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-API-Key': this.apiKey
      },
      body: JSON.stringify(envelope)
    });
    
    const result = await response.json();
    
    if (result.status === 'error') {
      throw new SynapError(result.error);
    }
    
    return result.payload;
  }
}
```

## Message Size Limits

```yaml
protocol:
  max_request_size_mb: 10
  max_response_size_mb: 10
  max_websocket_frame_kb: 64
```

Requests exceeding limits return 413 Payload Too Large.

## Timeouts

```yaml
protocol:
  request_timeout_secs: 30
  websocket_idle_timeout_secs: 300
  stream_chunk_timeout_ms: 5000
```

## Testing Protocol

### curl Examples

**KV SET**:
```bash
curl -X POST http://localhost:15500/api/v1/command \
  -H "Content-Type: application/json" \
  -H "X-API-Key: synap_key" \
  -d '{
    "request_id": "test-1",
    "command": "kv.set",
    "payload": {"key": "test", "value": "hello"}
  }'
```

**Queue CONSUME**:
```bash
curl -X POST http://localhost:15500/api/v1/command \
  -H "Content-Type: application/json" \
  -d '{
    "command": "queue.consume",
    "payload": {"queue": "tasks", "timeout": 5}
  }'
```

### WebSocket Testing

**websocat**:
```bash
websocat ws://localhost:15500/api/v1/ws

# Send subscribe command
{"command": "stream.subscribe", "payload": {"room": "test"}}
```

## See Also

- [REST_API.md](../api/REST_API.md) - Complete API reference
- [PROTOCOL_MESSAGES.md](../api/PROTOCOL_MESSAGES.md) - All message formats
- [TYPESCRIPT.md](../sdks/TYPESCRIPT.md) - TypeScript SDK implementation
- [ARCHITECTURE.md](../ARCHITECTURE.md) - System architecture

