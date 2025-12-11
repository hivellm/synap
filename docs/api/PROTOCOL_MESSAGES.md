# Protocol Messages Reference

## Message Types

Synap protocol defines four message types:

1. **Request** - Client to server command
2. **Response** - Server to client result
3. **Event** - Server-initiated notification (WebSocket/Stream)
4. **Batch** - Multiple commands in single request

## Request Message

### Structure

```typescript
interface RequestEnvelope {
  command: string;           // Command name (e.g., "kv.set")
  request_id: string;        // UUID v4 (required)
  payload: object;           // Command-specific parameters (required)
}
```

**Note**: The `type` and `version` fields are optional in the actual implementation. The minimal required format is:

```json
{
  "command": "kv.set",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "payload": {
    "key": "user:1001",
    "value": {"name": "Alice", "email": "alice@example.com"},
    "ttl": 3600
  }
}
```

### Full Example (with optional fields)

```json
{
  "type": "request",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "command": "kv.set",
  "version": "1.0",
  "payload": {
    "key": "user:1001",
    "value": {"name": "Alice", "email": "alice@example.com"},
    "ttl": 3600
  }
}
```

## Response Message

### Success Response

```typescript
interface ResponseEnvelope {
  success: boolean;          // true if operation succeeded
  request_id: string;        // Matches request
  payload?: object;          // Result data (if successful)
  error?: string;            // Error message (if failed)
}
```

**Note**: The actual StreamableHTTP response format uses `success` boolean instead of `status` field, and `error` is a string rather than an ErrorObject.

### Success Example

```json
{
  "type": "response",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "success",
  "payload": {
    "key": "user:1001",
    "success": true
  }
}
```

### Error Response

```typescript
interface ErrorObject {
  code: string;             // Error code (SNAKE_CASE)
  message: string;          // Human-readable message
  details?: object;         // Additional context
}
```

### Error Example

```json
{
  "type": "response",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "error",
  "error": {
    "code": "KEY_NOT_FOUND",
    "message": "Key 'user:999' does not exist",
    "details": {
      "key": "user:999",
      "suggestion": "Use kv.set to create the key first"
    }
  }
}
```

## Event Message

### Structure

Used for server-initiated notifications (WebSocket only):

```typescript
interface EventMessage {
  type: "event";
  event_id?: string;         // Optional event identifier
  offset?: number;           // Offset in stream (for streams)
  event_type?: string;       // Event type (for streams)
  topic?: string;            // Topic name (for pub/sub)
  data: any;                 // Event/message payload
  timestamp?: number;        // Unix timestamp
}
```

### Stream Event Example

```json
{
  "type": "event",
  "event_id": "evt_abc123",
  "offset": 42,
  "event_type": "message",
  "data": {
    "user": "alice",
    "text": "Hello!",
    "timestamp": "2025-10-15T19:30:00Z"
  },
  "timestamp": 1697410200
}
```

### Pub/Sub Message Example

```json
{
  "type": "event",
  "topic": "notifications.email.user",
  "data": {
    "to": "alice@example.com",
    "subject": "Welcome!",
    "body": "Thanks for signing up"
  },
  "timestamp": 1697410200
}
```

## Batch Message

### Request

```typescript
interface BatchRequest {
  type: "batch";
  request_id: string;
  commands: Array<{
    command: string;
    payload: object;
  }>;
}
```

### Example

```json
{
  "type": "batch",
  "request_id": "batch-001",
  "commands": [
    {
      "command": "kv.set",
      "payload": {"key": "user:1", "value": "Alice"}
    },
    {
      "command": "kv.set",
      "payload": {"key": "user:2", "value": "Bob"}
    },
    {
      "command": "kv.get",
      "payload": {"key": "user:1"}
    }
  ]
}
```

### Response

```typescript
interface BatchResponse {
  type: "batch_response";
  request_id: string;
  results: Array<{
    status: "success" | "error";
    payload?: object;
    error?: ErrorObject;
  }>;
}
```

### Example

```json
{
  "type": "batch_response",
  "request_id": "batch-001",
  "results": [
    {"status": "success", "payload": {"success": true}},
    {"status": "success", "payload": {"success": true}},
    {"status": "success", "payload": {"found": true, "value": "Alice"}}
  ]
}
```

## Command Payloads

### Key-Value Commands

**kv.set**:
```json
{
  "key": "string",
  "value": "any",
  "ttl": "integer?",
  "nx": "boolean?",
  "xx": "boolean?"
}
```

**kv.get**:
```json
{
  "key": "string"
}
```

**kv.del**:
```json
{
  "keys": ["string"]
}
```

**kv.incr**:
```json
{
  "key": "string",
  "amount": "integer?"
}
```

**kv.scan**:
```json
{
  "prefix": "string?",
  "cursor": "string?",
  "count": "integer?"
}
```

### Queue Commands

**queue.publish**:
```json
{
  "queue": "string",
  "message": "any",
  "priority": "integer?",
  "headers": "object?"
}
```

**queue.consume**:
```json
{
  "queue": "string",
  "timeout": "integer?",
  "ack_deadline": "integer?"
}
```

**queue.ack**:
```json
{
  "queue": "string",
  "message_id": "string"
}
```

**queue.nack**:
```json
{
  "queue": "string",
  "message_id": "string",
  "requeue": "boolean?"
}
```

### Stream Commands

**stream.publish**:
```json
{
  "room": "string",
  "event_type": "string",
  "data": "any",
  "metadata": "object?"
}
```

**stream.subscribe**:
```json
{
  "room": "string",
  "from_offset": "integer?",
  "replay": "boolean?"
}
```

**stream.history**:
```json
{
  "room": "string",
  "from_offset": "integer?",
  "to_offset": "integer?",
  "limit": "integer?"
}
```

### Pub/Sub Commands

**pubsub.publish**:
```json
{
  "topic": "string",
  "message": "any",
  "metadata": "object?"
}
```

**pubsub.subscribe**:
```json
{
  "topics": ["string"]
}
```

**pubsub.unsubscribe**:
```json
{
  "topics": ["string"]
}
```

## Error Codes Reference

### Client Errors (4xx)

| Code | HTTP | Description |
|------|------|-------------|
| INVALID_REQUEST | 400 | Malformed JSON or missing fields |
| INVALID_COMMAND | 400 | Unknown command |
| INVALID_PAYLOAD | 422 | Invalid parameter values |
| UNAUTHORIZED | 401 | Missing or invalid API key |
| FORBIDDEN | 403 | Insufficient permissions |
| KEY_NOT_FOUND | 404 | Key doesn't exist |
| QUEUE_NOT_FOUND | 404 | Queue doesn't exist |
| ROOM_NOT_FOUND | 404 | Room doesn't exist |
| TOPIC_NOT_FOUND | 404 | Topic has no subscribers |
| KEY_EXISTS | 409 | Key already exists (NX mode) |
| QUEUE_EXISTS | 409 | Queue already exists |
| MESSAGE_NOT_FOUND | 404 | Message ID not in pending |
| PAYLOAD_TOO_LARGE | 413 | Request exceeds size limit |
| RATE_LIMIT_EXCEEDED | 429 | Too many requests |

### Server Errors (5xx)

| Code | HTTP | Description |
|------|------|-------------|
| INTERNAL_ERROR | 500 | Unexpected server error |
| SERVICE_UNAVAILABLE | 503 | Server shutting down/overloaded |
| QUEUE_FULL | 507 | Queue at capacity |
| MEMORY_LIMIT_EXCEEDED | 507 | Memory limit reached |
| REPLICATION_ERROR | 500 | Replication failure |

## Serialization Formats

### JSON (Default)

**Content-Type**: `application/json`

**Advantages**:
- Human-readable
- Universal support
- Easy debugging

**Example**:
```json
{"key": "user:1", "value": {"name": "Alice", "age": 30}}
```

### MessagePack (Binary)

**Content-Type**: `application/msgpack`

**Advantages**:
- 30-50% smaller than JSON
- Faster serialization
- Binary data support

**Usage**:
```http
POST /api/v1/command HTTP/1.1
Content-Type: application/msgpack
Accept: application/msgpack
```

## Compression

### Request Compression

```http
POST /api/v1/command HTTP/1.1
Content-Encoding: gzip
```

### Response Compression

```http
Accept-Encoding: gzip
```

Server responds with:
```http
HTTP/1.1 200 OK
Content-Encoding: gzip
```

## Request ID Generation

### Format

UUID v4 format:
```
550e8400-e29b-41d4-a716-446655440000
```

### Client Responsibilities

1. Generate unique ID for each request
2. Use for request/response matching
3. Include in logs for debugging
4. Retry with same ID for idempotency

### Server Behavior

- Echo request_id in response
- Use for tracing and debugging
- Detect duplicate requests (idempotency)

## Idempotency

### Idempotent Operations

Safe to retry with same request_id:
- `kv.set`
- `kv.del`
- `queue.ack`
- `queue.nack`

### Non-Idempotent Operations

May cause duplicates if retried:
- `kv.incr` (use CAS instead)
- `queue.publish` (may create duplicates)

### Implementation

```rust
pub struct IdempotencyCache {
    cache: LruCache<RequestId, ResponseEnvelope>,
    ttl: Duration,
}

impl Server {
    async fn handle_request(&self, req: RequestEnvelope) -> ResponseEnvelope {
        // Check idempotency cache
        if let Some(cached) = self.idempotency.get(&req.request_id) {
            return cached.clone();
        }
        
        // Process request
        let response = self.process_command(req.command, req.payload).await;
        
        // Cache for idempotent operations
        if is_idempotent(&req.command) {
            self.idempotency.insert(req.request_id, response.clone());
        }
        
        response
    }
}
```

## Connection Management

### Keep-Alive

```http
Connection: keep-alive
Keep-Alive: timeout=60, max=1000
```

### Connection Pooling

Clients should reuse connections:
- Pool size: 5-20 connections
- Connection timeout: 60 seconds
- Idle timeout: 300 seconds

### WebSocket Heartbeat

```json
// Server → Client (every 30s)
{"type": "ping", "timestamp": 1697410800}

// Client → Server
{"type": "pong", "timestamp": 1697410800}
```

Disconnect after 3 missed pongs.

## See Also

- [STREAMABLE_HTTP.md](../protocol/STREAMABLE_HTTP.md) - Full protocol spec
- [REST_API.md](REST_API.md) - API endpoint reference
- [ERROR_HANDLING.md](ERROR_HANDLING.md) - Error handling guide

