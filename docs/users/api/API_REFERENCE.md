---
title: REST API Reference
module: api
id: api-reference
order: 1
description: Complete REST API endpoint reference
tags: [api, rest, endpoints, reference]
---

# REST API Reference

Complete reference for all Synap REST API endpoints.

## Base URL

```
http://localhost:15500
```

## System Endpoints

### Health Check

**GET** `/health`

Returns server health status.

**Response:**
```json
{
  "status": "healthy",
  "uptime_secs": 12345
}
```

### Server Info

**GET** `/info`

Returns server information.

**Response:**
```json
{
  "version": "0.8.1",
  "uptime_secs": 12345,
  "memory_usage_bytes": 4294967296
}
```

### Metrics

**GET** `/metrics`

Returns Prometheus metrics.

## Key-Value Store

### Set Key

**POST** `/kv/set`

Set a key-value pair.

**Request:**
```json
{
  "key": "user:1",
  "value": "John Doe",
  "ttl": 3600
}
```

**Response:**
```json
{
  "success": true
}
```

### Get Key

**GET** `/kv/get/{key}`

Get value by key.

**Response:**
```
"John Doe"
```

### Delete Key

**DELETE** `/kv/del/{key}`

Delete a key.

**Response:**
```json
{
  "deleted": true
}
```

### Statistics

**GET** `/kv/stats`

Get KV store statistics.

**Response:**
```json
{
  "total_keys": 42,
  "memory_bytes": 8192,
  "eviction_policy": "lru"
}
```

## Message Queues

### Create Queue

**POST** `/queue/{name}`

Create a new queue.

**Request:**
```json
{
  "max_depth": 1000,
  "ack_deadline_secs": 30,
  "default_max_retries": 3
}
```

### Publish Message

**POST** `/queue/{name}/publish`

Publish a message to queue.

**Request:**
```json
{
  "payload": [72, 101, 108, 108, 111],
  "priority": 5,
  "max_retries": 3
}
```

### Consume Message

**GET** `/queue/{name}/consume/{consumer}`

Consume a message from queue.

**Response:**
```json
{
  "message_id": "abc-123",
  "payload": [72, 101, 108, 108, 111],
  "priority": 5,
  "retry_count": 0
}
```

### Acknowledge Message

**POST** `/queue/{name}/ack`

Acknowledge a message.

**Request:**
```json
{
  "message_id": "abc-123"
}
```

### Negative Acknowledge

**POST** `/queue/{name}/nack`

Reject a message (will retry).

**Request:**
```json
{
  "message_id": "abc-123"
}
```

## Event Streams

### Create Stream

**POST** `/stream/{name}`

Create a new stream.

### Publish Event

**POST** `/stream/{name}/publish`

Publish an event to stream.

**Request:**
```json
{
  "event": "user.signup",
  "data": "New user registered"
}
```

### Consume Events

**GET** `/stream/{name}/consume/{consumer}`

Consume events from stream.

**Query Parameters:**
- `from_offset` - Starting offset (default: 0)
- `limit` - Maximum events (default: 10)

**Response:**
```json
[
  {
    "offset": 0,
    "event": "user.signup",
    "data": "New user registered",
    "timestamp": 1234567890
  }
]
```

## Pub/Sub

### Publish to Topic

**POST** `/pubsub/{topic}/publish`

Publish message to topic.

**Request:**
```json
{
  "message": "New order received"
}
```

### WebSocket Subscribe

**WebSocket** `/pubsub/ws`

Subscribe to topics via WebSocket.

**Query Parameters:**
- `topics` - Comma-separated topic list (supports wildcards)

**Example:**
```
ws://localhost:15500/pubsub/ws?topics=notifications.email,events.order.*
```

## Related Topics

- [Authentication](./AUTHENTICATION.md) - Authentication and security
- [StreamableHTTP Protocol](./STREAMABLE_HTTP.md) - Protocol documentation
- [MCP Integration](./MCP.md) - Model Context Protocol

