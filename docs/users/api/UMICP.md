---
title: UMICP Protocol
module: api
id: umicp-protocol
order: 5
description: Universal Matrix Inter-Communication Protocol
tags: [api, umicp, protocol, binary, high-performance]
---

# UMICP Protocol

Complete guide to using the Universal Matrix Inter-Communication Protocol (UMICP) with Synap.

## Overview

Synap supports **UMICP (Universal Matrix Intelligent Communication Protocol) v0.2.3**, providing a high-performance binary protocol for AI model interoperability and inter-process communication.

**Version**: 0.2.3  
**Status**: ✅ Production Ready

## Features

- ✅ **Envelope-based Communication**: Structured binary protocol with validation
- ✅ **Tool Discovery**: Auto-discovery of operations
- ✅ **MCP Compatible**: Converts UMICP → MCP → UMICP
- ✅ **Native JSON Types**: Full support for numbers, booleans, arrays, objects
- ✅ **HTTP/2 Transport**: High-performance streaming over HTTP
- ✅ **Zero Duplication**: Wraps existing MCP handlers

## Endpoints

### Message Handler

**Endpoint**: `POST /umicp`  
**Content-Type**: `application/json`

Accepts UMICP envelopes and routes to appropriate MCP handlers.

**Example Request:**
```json
{
  "from": "client-001",
  "to": "synap-server",
  "operation": "Request",
  "message_id": "msg-12345",
  "capabilities": {
    "operation": "synap_kv_set",
    "key": "user:001",
    "value": "John Doe"
  }
}
```

**Example Response:**
```json
{
  "from": "synap-server",
  "to": "client-001",
  "operation": "Data",
  "message_id": "resp-msg-12345",
  "capabilities": {
    "status": "success",
    "result": {"success": true},
    "original_message_id": "msg-12345"
  }
}
```

### Discovery Handler

**Endpoint**: `GET /umicp/discover`

Returns all available operations in UMICP format.

**Example Response:**
```json
{
  "protocol": "UMICP",
  "version": "0.2.3",
  "server_info": {
    "server": "synap-server",
    "version": "0.8.1",
    "protocol": "UMICP/2.0",
    "features": [
      "key-value-store",
      "message-queues",
      "event-streams",
      "pub-sub",
      "persistence",
      "replication",
      "mcp-compatible"
    ],
    "operations_count": 8,
    "mcp_compatible": true
  },
  "operations": [ /* operation schemas */ ],
  "total_operations": 8
}
```

## Available Operations

Synap exposes **8 core operations** via UMICP:

| Operation | Type | Description |
|-----------|------|-------------|
| `synap_kv_get` | Request | Retrieve value from KV store |
| `synap_kv_set` | Request | Store key-value pair |
| `synap_kv_delete` | Request | Delete key from store |
| `synap_kv_scan` | Request | Scan keys by prefix |
| `synap_queue_publish` | Request | Publish message to queue |
| `synap_queue_consume` | Request | Consume message from queue |
| `synap_stream_publish` | Request | Publish event to stream |
| `synap_stream_consume` | Request | Consume events from stream |

## Usage Examples

### Key-Value Operations

**Set Key:**
```json
{
  "from": "client-001",
  "to": "synap-server",
  "operation": "Request",
  "message_id": "msg-001",
  "capabilities": {
    "operation": "synap_kv_set",
    "key": "user:1",
    "value": "John Doe",
    "ttl": 3600
  }
}
```

**Get Key:**
```json
{
  "from": "client-001",
  "to": "synap-server",
  "operation": "Request",
  "message_id": "msg-002",
  "capabilities": {
    "operation": "synap_kv_get",
    "key": "user:1"
  }
}
```

### Queue Operations

**Publish Message:**
```json
{
  "from": "client-001",
  "to": "synap-server",
  "operation": "Request",
  "message_id": "msg-003",
  "capabilities": {
    "operation": "synap_queue_publish",
    "queue": "jobs",
    "message": "Hello",
    "priority": 5
  }
}
```

**Consume Message:**
```json
{
  "from": "client-001",
  "to": "synap-server",
  "operation": "Request",
  "message_id": "msg-004",
  "capabilities": {
    "operation": "synap_queue_consume",
    "queue": "jobs",
    "consumer": "worker-1"
  }
}
```

## Envelope Format

### Request Envelope

```json
{
  "from": "client-id",
  "to": "synap-server",
  "operation": "Request",
  "message_id": "unique-id",
  "capabilities": {
    "operation": "operation-name",
    "param1": "value1",
    "param2": "value2"
  }
}
```

### Response Envelope

```json
{
  "from": "synap-server",
  "to": "client-id",
  "operation": "Data",
  "message_id": "response-id",
  "capabilities": {
    "status": "success",
    "result": { /* operation result */ },
    "original_message_id": "request-id"
  }
}
```

### Error Response

```json
{
  "from": "synap-server",
  "to": "client-id",
  "operation": "Error",
  "message_id": "response-id",
  "capabilities": {
    "status": "error",
    "error": {
      "code": "ERROR_CODE",
      "message": "Error message"
    },
    "original_message_id": "request-id"
  }
}
```

## MCP Compatibility

UMICP is fully compatible with MCP. All MCP tools are available via UMICP:

```json
{
  "operation": "synap_kv_get",
  "key": "user:1"
}
```

This is equivalent to the MCP call:
```json
{
  "method": "tools/call",
  "params": {
    "name": "synap_kv_get",
    "arguments": {
      "key": "user:1"
    }
  }
}
```

## Best Practices

### Use Unique Message IDs

```javascript
function generateMessageId() {
  return 'msg-' + Date.now() + '-' + Math.random().toString(36).substr(2, 9);
}
```

### Handle Errors

```javascript
if (response.capabilities.status === 'error') {
  console.error('Error:', response.capabilities.error.message);
  // Handle error
}
```

### Discover Operations

Always discover available operations on connection:

```bash
curl http://localhost:15500/umicp/discover
```

## Related Topics

- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [MCP Integration](./MCP.md) - Model Context Protocol
- [StreamableHTTP Protocol](./STREAMABLE_HTTP.md) - Protocol documentation

