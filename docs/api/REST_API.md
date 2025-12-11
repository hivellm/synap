# REST API Reference

## Base URL

**REST API Endpoints**:
```
http://localhost:15500
```

**StreamableHTTP Endpoint**:
```
http://localhost:15500/api/v1
```

## Request Format Compatibility

Synap supports multiple request formats for improved SDK compatibility:

### Hash Operations - Multiple Set (HMSET)

**REST Endpoint**: `POST /hash/{key}/mset`

**Object Format** (original):
```json
{
  "fields": {
    "name": "Alice",
    "age": 30,
    "email": "alice@example.com"
  }
}
```

**Array Format** (SDK compatibility):
```json
[
  {"field": "name", "value": "Alice"},
  {"field": "age", "value": 30},
  {"field": "email", "value": "alice@example.com"}
]
```

### String Operations - Set if Not Exists (MSETNX)

**REST Endpoint**: `POST /kv/msetnx`

**Object Format** (preferred):
```json
{
  "key": "user:1001",
  "value": {"name": "Alice", "age": 30}
}
```

**Tuple Format** (backward compatible):
```json
["user:1001", {"name": "Alice", "age": 30}]
```

### List Operations - Pop (LPOP/RPOP)

**REST Endpoint**: `POST /list/{key}/lpop` or `POST /list/{key}/rpop`

**With Count**:
```json
{
  "count": 3
}
```

**Without Count** (defaults to 1):
```json
{}
```

### Sorted Set Operations - Add (ZADD)

**REST Endpoint**: `POST /sortedset/{key}/zadd`

**Single Member**:
```json
{
  "member": "player1",
  "score": 100.5
}
```

**Multiple Members** (Redis-compatible):
```json
{
  "members": [
    {"member": "player1", "score": 100.5},
    {"member": "player2", "score": 200.0}
  ]
}
```

### Memory Usage

**REST Endpoint**: `GET /memory/{key}/usage`

Returns `{"bytes": 0, "human": "0B"}` for non-existent keys instead of 404 error.

## Authentication

All requests require authentication via API key:

```http
X-API-Key: synap_your_api_key_here
```

Or Bearer token:

```http
Authorization: Bearer your_jwt_token_here
```

## Common Response Format

### Success Response

```json
{
  "type": "response",
  "request_id": "uuid",
  "status": "success",
  "payload": {
    // Command-specific data
  }
}
```

### Error Response

```json
{
  "type": "response",
  "request_id": "uuid",
  "status": "error",
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable error message",
    "details": {}
  }
}
```

## Key-Value Store API

### SET - Store Key-Value Pair

`POST /api/v1/command`

```json
{
  "command": "kv.set",
  "payload": {
    "key": "user:1001",
    "value": {"name": "Alice", "age": 30},
    "ttl": 3600,
    "nx": false,
    "xx": false
  }
}
```

**Parameters**:
- `key` (string): Key name
- `value` (any): Value to store
- `ttl` (integer, optional): Time-to-live in seconds
- `nx` (boolean, optional): Only set if not exists
- `xx` (boolean, optional): Only set if exists

**Response**:
```json
{
  "status": "success",
  "payload": {
    "key": "user:1001",
    "success": true,
    "previous": null
  }
}
```

### GET - Retrieve Value

`POST /api/v1/command`

```json
{
  "command": "kv.get",
  "payload": {
    "key": "user:1001"
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "found": true,
    "value": {"name": "Alice", "age": 30},
    "ttl": 3542
  }
}
```

### DEL - Delete Keys

`POST /api/v1/command`

```json
{
  "command": "kv.del",
  "payload": {
    "keys": ["user:1001", "user:1002"]
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "deleted": 2
  }
}
```

### INCR - Increment Value

`POST /api/v1/command`

```json
{
  "command": "kv.incr",
  "payload": {
    "key": "counter:views",
    "amount": 1
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "value": 42
  }
}
```

### SCAN - Scan Keys

`POST /api/v1/command`

```json
{
  "command": "kv.scan",
  "payload": {
    "prefix": "user:",
    "cursor": null,
    "count": 100
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "keys": ["user:1", "user:2", "user:3"],
    "cursor": "next-page-token",
    "has_more": true
  }
}
```

## Queue System API

### PUBLISH - Add Message to Queue

`POST /api/v1/command`

```json
{
  "command": "queue.publish",
  "payload": {
    "queue": "tasks",
    "message": {
      "type": "process_video",
      "video_id": "vid_123"
    },
    "priority": 8,
    "headers": {"source": "web-app"}
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "message_id": "msg_abc123",
    "position": 3
  }
}
```

### CONSUME - Get Message from Queue

`POST /api/v1/command`

```json
{
  "command": "queue.consume",
  "payload": {
    "queue": "tasks",
    "timeout": 10,
    "ack_deadline": 60
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "message_id": "msg_abc123",
    "message": {"type": "process_video", "video_id": "vid_123"},
    "priority": 8,
    "retry_count": 0,
    "headers": {"source": "web-app"}
  }
}
```

### ACK - Acknowledge Message

`POST /api/v1/command`

```json
{
  "command": "queue.ack",
  "payload": {
    "queue": "tasks",
    "message_id": "msg_abc123"
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "success": true
  }
}
```

### NACK - Negative Acknowledge

`POST /api/v1/command`

```json
{
  "command": "queue.nack",
  "payload": {
    "queue": "tasks",
    "message_id": "msg_abc123",
    "requeue": true
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "success": true,
    "action": "requeued"
  }
}
```

### QUEUE STATS

`POST /api/v1/command`

```json
{
  "command": "queue.stats",
  "payload": {
    "queue": "tasks"
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "queue": "tasks",
    "depth": 150,
    "consumers": 5,
    "published_total": 10000,
    "consumed_total": 9850,
    "acked_total": 9700,
    "oldest_message_age_secs": 45
  }
}
```

## Event Stream API

### PUBLISH - Publish Event to Room

`POST /api/v1/command`

```json
{
  "command": "stream.publish",
  "payload": {
    "room": "chat-room-1",
    "event_type": "message",
    "data": {
      "user": "alice",
      "text": "Hello everyone!"
    }
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "event_id": "evt_xyz789",
    "offset": 42,
    "subscribers_notified": 5
  }
}
```

### SUBSCRIBE - Subscribe to Room (WebSocket)

`WebSocket: ws://localhost:15500/api/v1/ws`

**Subscribe Message**:
```json
{
  "command": "stream.subscribe",
  "payload": {
    "room": "chat-room-1",
    "from_offset": null,
    "replay": false
  }
}
```

**Event Stream**:
```json
{"type": "event", "offset": 42, "event_type": "message", "data": {...}}
{"type": "event", "offset": 43, "event_type": "join", "data": {...}}
```

### HISTORY - Get Event History

`POST /api/v1/command`

```json
{
  "command": "stream.history",
  "payload": {
    "room": "chat-room-1",
    "from_offset": 30,
    "limit": 10
  }
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "events": [
      {"offset": 30, "event_type": "message", "data": {...}},
      {"offset": 31, "event_type": "join", "data": {...}}
    ],
    "oldest_offset": 1,
    "newest_offset": 42
  }
}
```

## Pub/Sub API

### PUBLISH - Publish to Topic

**REST Endpoint**: `POST /pubsub/{topic}/publish`

**Request Body** (supports both `payload` and `data` fields):
```json
{
  "payload": {
    "to": "alice@example.com",
    "subject": "Welcome!"
  },
  "metadata": {
    "source": "web-app",
    "priority": "high"
  }
}
```

**Alternative format** (SDK compatibility):
```json
{
  "data": {
    "to": "alice@example.com",
    "subject": "Welcome!"
  }
}
```

**Note**: The `payload` field is preferred, but `data` is accepted for SDK compatibility.

**Response**:
```json
{
  "subscribers_notified": 3
}
```

**StreamableHTTP Format**: `POST /api/v1/command`

```json
{
  "command": "pubsub.publish",
  "payload": {
    "topic": "notifications.email.user",
    "message": {
      "to": "alice@example.com",
      "subject": "Welcome!"
    }
  }
}
```

### SUBSCRIBE - Subscribe to Topics (WebSocket)

`WebSocket: ws://localhost:15500/api/v1/ws`

**Subscribe Message**:
```json
{
  "command": "pubsub.subscribe",
  "payload": {
    "topics": ["notifications.email.*", "events.user.#"]
  }
}
```

**Message Stream**:
```json
{"type": "message", "topic": "notifications.email.user", "message": {...}}
{"type": "message", "topic": "events.user.login", "message": {...}}
```

## Admin API

### STATS - System Statistics

`POST /api/v1/command`

```json
{
  "command": "admin.stats"
}
```

**Response**:
```json
{
  "status": "success",
  "payload": {
    "server": {
      "version": "0.1.0",
      "uptime_secs": 86400,
      "role": "master"
    },
    "memory": {
      "used_bytes": 536870912,
      "limit_bytes": 4294967296
    },
    "kv": {
      "total_keys": 1000000,
      "operations_per_sec": 50000
    },
    "queues": {
      "total_queues": 50,
      "total_messages": 5000
    },
    "streams": {
      "total_rooms": 100,
      "total_subscribers": 500
    },
    "pubsub": {
      "total_topics": 200,
      "total_subscribers": 1000
    }
  }
}
```

### INFO - Server Information

`GET /health`

**Response**:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "role": "master",
  "uptime_secs": 86400,
  "components": {
    "kv_store": "operational",
    "queue_system": "operational",
    "event_stream": "operational",
    "pubsub": "operational",
    "replication": "operational"
  }
}
```

### METRICS - Prometheus Metrics

`GET /metrics`

**Response** (Prometheus format):
```prometheus
# HELP synap_operations_total Total operations by type
# TYPE synap_operations_total counter
synap_operations_total{component="kv",operation="get"} 5000000
synap_operations_total{component="kv",operation="set"} 1000000
synap_operations_total{component="queue",operation="publish"} 100000

# HELP synap_memory_bytes Memory usage in bytes
# TYPE synap_memory_bytes gauge
synap_memory_bytes 536870912

# HELP synap_replication_lag_ms Replication lag in milliseconds
# TYPE synap_replication_lag_ms gauge
synap_replication_lag_ms{replica="replica-1"} 5
```

## Batch Operations

### BATCH - Execute Multiple Commands

`POST /api/v1/command`

```json
{
  "type": "batch",
  "request_id": "batch-001",
  "commands": [
    {
      "command": "kv.set",
      "payload": {"key": "user:1", "value": "data1"}
    },
    {
      "command": "kv.set",
      "payload": {"key": "user:2", "value": "data2"}
    },
    {
      "command": "kv.get",
      "payload": {"key": "user:1"}
    }
  ]
}
```

**Response**:
```json
{
  "type": "batch_response",
  "request_id": "batch-001",
  "results": [
    {"status": "success", "payload": {"success": true}},
    {"status": "success", "payload": {"success": true}},
    {"status": "success", "payload": {"found": true, "value": "data1"}}
  ]
}
```

## Rate Limiting

### Headers

**Request**:
```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
```

**Response when limited**:
```http
HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1697410920

{
  "status": "error",
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded. Try again in 60 seconds."
  }
}
```

## WebSocket API

### Connection

`GET /api/v1/ws`

**Upgrade Request**:
```http
GET /api/v1/ws HTTP/1.1
Host: localhost:15500
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
X-API-Key: synap_key
```

### Commands over WebSocket

Same command format as HTTP:

```json
{
  "request_id": "ws-001",
  "command": "stream.subscribe",
  "payload": {"room": "chat-1"}
}
```

### Server-Initiated Messages

```json
{
  "type": "event",
  "offset": 42,
  "event_type": "message",
  "data": {...}
}
```

## Complete API Reference

### Key-Value Operations

| Command | Description | Parameters |
|---------|-------------|------------|
| `kv.set` | Store key-value | key, value, ttl, nx, xx |
| `kv.get` | Retrieve value | key |
| `kv.del` | Delete keys | keys[] |
| `kv.exists` | Check existence | key |
| `kv.incr` | Increment | key, amount |
| `kv.decr` | Decrement | key, amount |
| `kv.expire` | Set TTL | key, ttl |
| `kv.ttl` | Get remaining TTL | key |
| `kv.scan` | Scan keys | prefix, cursor, count |
| `kv.mset` | Set multiple | pairs[] |
| `kv.mget` | Get multiple | keys[] |

### Queue Operations

| Command | Description | Parameters |
|---------|-------------|------------|
| `queue.create` | Create queue | queue, config |
| `queue.delete` | Delete queue | queue |
| `queue.publish` | Add message | queue, message, priority |
| `queue.consume` | Get message | queue, timeout, ack_deadline |
| `queue.ack` | Acknowledge | queue, message_id |
| `queue.nack` | Negative ack | queue, message_id, requeue |
| `queue.purge` | Clear queue | queue |
| `queue.stats` | Get statistics | queue |
| `queue.list` | List queues | - |

### Event Stream Operations

| Command | Description | Parameters |
|---------|-------------|------------|
| `stream.publish` | Publish event | room, event_type, data |
| `stream.subscribe` | Subscribe (WS) | room, from_offset, replay |
| `stream.unsubscribe` | Unsubscribe | room |
| `stream.history` | Get history | room, from_offset, limit |
| `stream.rooms` | List rooms | - |
| `stream.stats` | Room statistics | room |

### Pub/Sub Operations

| Command | Description | Parameters |
|---------|-------------|------------|
| `pubsub.publish` | Publish message | topic, message |
| `pubsub.subscribe` | Subscribe (WS) | topics[] |
| `pubsub.unsubscribe` | Unsubscribe | topics[] |
| `pubsub.topics` | List topics | pattern |
| `pubsub.stats` | Get statistics | - |

### Admin Operations

| Command | Description | Parameters |
|---------|-------------|------------|
| `admin.stats` | System stats | - |
| `admin.config` | Get config | - |
| `admin.health` | Health check | - |
| `admin.shutdown` | Graceful shutdown | - |

### Replication Operations

| Command | Description | Parameters |
|---------|-------------|------------|
| `replication.status` | Get status | - |
| `replication.promote` | Promote replica | force |
| `replication.resync` | Force resync | - |

## HTTP Status Codes

| Status | Code | Usage |
|--------|------|-------|
| OK | 200 | Successful operation |
| Created | 201 | Resource created |
| No Content | 204 | Successful deletion |
| Bad Request | 400 | Invalid request format |
| Unauthorized | 401 | Invalid/missing API key |
| Forbidden | 403 | Insufficient permissions |
| Not Found | 404 | Resource not found |
| Conflict | 409 | Resource already exists |
| Payload Too Large | 413 | Request too large |
| Unprocessable Entity | 422 | Valid format, invalid data |
| Too Many Requests | 429 | Rate limit exceeded |
| Internal Server Error | 500 | Server error |
| Service Unavailable | 503 | Server overloaded |
| Insufficient Storage | 507 | Memory limit exceeded |

## Example Workflows

### Session Management

```bash
# 1. Create session
curl -X POST http://localhost:15500/api/v1/command \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $API_KEY" \
  -d '{
    "command": "kv.set",
    "payload": {
      "key": "session:abc123",
      "value": {"user_id": 1, "logged_in": true},
      "ttl": 3600
    }
  }'

# 2. Get session
curl -X POST http://localhost:15500/api/v1/command \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $API_KEY" \
  -d '{
    "command": "kv.get",
    "payload": {"key": "session:abc123"}
  }'

# 3. Delete session (logout)
curl -X POST http://localhost:15500/api/v1/command \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $API_KEY" \
  -d '{
    "command": "kv.del",
    "payload": {"keys": ["session:abc123"]}
  }'
```

### Task Processing

```bash
# 1. Publish task
curl -X POST http://localhost:15500/api/v1/command \
  -d '{
    "command": "queue.publish",
    "payload": {
      "queue": "tasks",
      "message": {"type": "send_email", "to": "user@example.com"},
      "priority": 5
    }
  }'

# 2. Consume task
curl -X POST http://localhost:15500/api/v1/command \
  -d '{
    "command": "queue.consume",
    "payload": {"queue": "tasks", "timeout": 30}
  }'

# 3. Acknowledge completion
curl -X POST http://localhost:15500/api/v1/command \
  -d '{
    "command": "queue.ack",
    "payload": {"queue": "tasks", "message_id": "msg_123"}
  }'
```

## HiveHub.Cloud Integration (SaaS Mode)

### GET /hub/quota

Get current quota usage and limits for authenticated user.

**Requires Hub Integration**: This endpoint is only available when Synap is integrated with HiveHub.Cloud (`hub.enabled: true`).

**Authentication**: Required - Hub access key via `Authorization: Bearer` or `X-Hub-Access-Key` header

**Request**:
```bash
curl -X GET http://localhost:15500/hub/quota \
  -H "Authorization: Bearer sk_live_a1b2c3d4e5f6..."
```

**Response**:
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "plan": "Pro",
  "quotas": {
    "storage": {
      "limit": 10737418240,
      "used": 5368709120,
      "available": 5368709120,
      "percentage": 50.0
    },
    "operations": {
      "limit": 10000000,
      "used": 3456789,
      "available": 6543211,
      "percentage": 34.6,
      "resets_at": "2025-02-01T00:00:00Z"
    },
    "connections": {
      "limit": 100,
      "active": 12,
      "available": 88,
      "percentage": 12.0
    }
  },
  "restrictions": {
    "max_ttl_seconds": 2592000,
    "max_payload_bytes": 10485760,
    "max_batch_size": 1000
  }
}
```

**Response Fields**:

- `user_id` (string): UUID of the authenticated user
- `plan` (string): User's subscription plan (`Free`, `Pro`, `Enterprise`)
- `quotas.storage` (object): Storage quota information
  - `limit` (number): Maximum storage in bytes
  - `used` (number): Current storage used in bytes
  - `available` (number): Remaining storage in bytes
  - `percentage` (number): Usage percentage (0-100)
- `quotas.operations` (object): Monthly operation quota
  - `limit` (number): Maximum operations per month
  - `used` (number): Operations used this month
  - `available` (number): Remaining operations
  - `percentage` (number): Usage percentage (0-100)
  - `resets_at` (string): ISO 8601 timestamp when quota resets
- `quotas.connections` (object): Connection quota
  - `limit` (number): Maximum concurrent connections
  - `active` (number): Currently active connections
  - `available` (number): Available connection slots
  - `percentage` (number): Usage percentage (0-100)
- `restrictions` (object): Plan-based restrictions
  - `max_ttl_seconds` (number): Maximum TTL for resources
  - `max_payload_bytes` (number): Maximum payload size
  - `max_batch_size` (number): Maximum batch operation size

**Status Codes**:

- `200 OK`: Quota information returned successfully
- `401 Unauthorized`: Invalid or missing access key
- `500 Internal Server Error`: Failed to fetch quota from Hub API

**Example - Free Plan**:
```json
{
  "user_id": "123e4567-e89b-12d3-a456-426614174000",
  "plan": "Free",
  "quotas": {
    "storage": {
      "limit": 104857600,
      "used": 52428800,
      "available": 52428800,
      "percentage": 50.0
    },
    "operations": {
      "limit": 100000,
      "used": 45000,
      "available": 55000,
      "percentage": 45.0,
      "resets_at": "2025-02-01T00:00:00Z"
    },
    "connections": {
      "limit": 10,
      "active": 3,
      "available": 7,
      "percentage": 30.0
    }
  },
  "restrictions": {
    "max_ttl_seconds": 86400,
    "max_payload_bytes": 1048576,
    "max_batch_size": 100
  }
}
```

**Example - Enterprise Plan**:
```json
{
  "user_id": "789e0123-e45f-67g8-h901-234567890abc",
  "plan": "Enterprise",
  "quotas": {
    "storage": {
      "limit": 1099511627776,
      "used": 549755813888,
      "available": 549755813888,
      "percentage": 50.0
    },
    "operations": {
      "limit": null,
      "used": 50000000,
      "available": null,
      "percentage": 0.0,
      "resets_at": null
    },
    "connections": {
      "limit": 1000,
      "active": 250,
      "available": 750,
      "percentage": 25.0
    }
  },
  "restrictions": {
    "max_ttl_seconds": 31536000,
    "max_payload_bytes": 104857600,
    "max_batch_size": 10000
  }
}
```

**Error Responses**:

**401 Unauthorized** - Invalid access key:
```json
{
  "error": "Unauthorized",
  "message": "Invalid or missing access key",
  "code": "INVALID_ACCESS_KEY"
}
```

**500 Internal Server Error** - Hub API failure:
```json
{
  "error": "InternalServerError",
  "message": "Failed to fetch quota from Hub API: Connection timeout",
  "code": "HUB_API_ERROR"
}
```

**Plan Comparison**:

| Feature                  | Free         | Pro           | Enterprise    |
|--------------------------|--------------|---------------|---------------|
| Storage Limit            | 100 MB       | 10 GB         | 1 TB          |
| Operations/Month         | 100,000      | 10,000,000    | Unlimited     |
| Concurrent Connections   | 10           | 100           | 1,000         |
| Max TTL                  | 24 hours     | 30 days       | 365 days      |
| Max Payload Size         | 1 MB         | 10 MB         | 100 MB        |
| Max Batch Size           | 100          | 1,000         | 10,000        |
| Rate Limit               | 10 req/s     | 100 req/s     | 1,000 req/s   |

**Notes**:

- Quota data is cached for 60 seconds for performance
- `operations.limit` is `null` for Enterprise plan (unlimited)
- `operations.percentage` is always 0.0 for Enterprise plan
- Storage includes all data: queues, streams, KV store, data structures
- Operations count includes all read/write operations
- Connections count active persistent HTTP/2 connections
- Quota exceeded results in `429 Too Many Requests` responses

**Related Documentation**:

- [HUB_INTEGRATION.md](../specs/HUB_INTEGRATION.md) - Complete Hub integration specification
- [QUOTA_MANAGEMENT.md](../specs/QUOTA_MANAGEMENT.md) - Detailed quota system documentation
- [ACCESS_KEYS.md](../specs/ACCESS_KEYS.md) - Access key authentication guide
- [HUB_CONFIGURATION.md](../guides/HUB_CONFIGURATION.md) - Setup guide

## See Also

- [STREAMABLE_HTTP.md](../protocol/STREAMABLE_HTTP.md) - Protocol specification
- [PROTOCOL_MESSAGES.md](PROTOCOL_MESSAGES.md) - Message format details
- [TYPESCRIPT.md](../sdks/TYPESCRIPT.md) - TypeScript SDK
- [PYTHON.md](../sdks/PYTHON.md) - Python SDK

