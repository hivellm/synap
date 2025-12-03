---
title: StreamableHTTP Protocol
module: api
id: streamable-http
order: 3
description: StreamableHTTP protocol documentation
tags: [api, protocol, streamable-http, http]
---

# StreamableHTTP Protocol

Complete guide to the StreamableHTTP protocol in Synap.

## Overview

StreamableHTTP is a lightweight protocol built on top of HTTP/1.1 and HTTP/2 that enables request-response and streaming communication patterns for Synap.

## Design Principles

1. **HTTP-Based**: Uses standard HTTP as transport
2. **Universal**: Works with any HTTP client
3. **Debuggable**: Human-readable JSON messages
4. **Streaming**: Supports chunked transfer encoding
5. **Upgradeable**: Can upgrade to WebSocket

## Endpoint

**Base URL:**
```
http://localhost:15500/api/v1
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

**Fields:**
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

**Fields:**
- `type` (string): Always "response"
- `request_id` (string): Matches request ID
- `status` (string): "success" or "error"
- `payload` (object): Response data or error details

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

## Usage Examples

### Key-Value Operations

**Set Key:**
```bash
curl -X POST http://localhost:15500/api/v1 \
  -H "Content-Type: application/json" \
  -d '{
    "type": "request",
    "request_id": "req-123",
    "command": "kv.set",
    "version": "1.0",
    "payload": {
      "key": "user:1",
      "value": "John Doe",
      "ttl": 3600
    }
  }'
```

**Get Key:**
```bash
curl -X POST http://localhost:15500/api/v1 \
  -H "Content-Type: application/json" \
  -d '{
    "type": "request",
    "request_id": "req-124",
    "command": "kv.get",
    "version": "1.0",
    "payload": {
      "key": "user:1"
    }
  }'
```

### Queue Operations

**Publish Message:**
```bash
curl -X POST http://localhost:15500/api/v1 \
  -H "Content-Type: application/json" \
  -d '{
    "type": "request",
    "request_id": "req-125",
    "command": "queue.publish",
    "version": "1.0",
    "payload": {
      "queue": "jobs",
      "message": "Hello",
      "priority": 5
    }
  }'
```

## Command Reference

### Key-Value Commands

- `kv.set` - Set key-value pair
- `kv.get` - Get value by key
- `kv.delete` - Delete key
- `kv.exists` - Check if key exists
- `kv.mset` - Multiple set
- `kv.mget` - Multiple get

### Queue Commands

- `queue.create` - Create queue
- `queue.publish` - Publish message
- `queue.consume` - Consume message
- `queue.ack` - Acknowledge message
- `queue.nack` - Negative acknowledge

### Stream Commands

- `stream.create` - Create stream
- `stream.publish` - Publish event
- `stream.consume` - Consume events

### Pub/Sub Commands

- `pubsub.publish` - Publish to topic
- `pubsub.subscribe` - Subscribe to topics

## WebSocket Upgrade

### Upgrade to WebSocket

```javascript
const ws = new WebSocket('ws://localhost:15500/api/v1');

ws.onopen = () => {
  // Send request
  ws.send(JSON.stringify({
    type: "request",
    request_id: "req-123",
    command: "kv.get",
    version: "1.0",
    payload: { key: "user:1" }
  }));
};

ws.onmessage = (event) => {
  const response = JSON.parse(event.data);
  console.log('Response:', response);
};
```

## Best Practices

### Use Unique Request IDs

```javascript
function generateRequestId() {
  return 'req-' + Date.now() + '-' + Math.random().toString(36).substr(2, 9);
}
```

### Handle Errors

```javascript
if (response.status === 'error') {
  console.error('Error:', response.error.message);
  // Handle error
}
```

### Reuse Connections

For multiple requests, reuse HTTP connections or use WebSocket.

## Related Topics

- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [MCP Integration](./MCP.md) - Model Context Protocol
- [UMICP Protocol](./UMICP.md) - Universal Matrix Inter-Communication Protocol

