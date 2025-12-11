---
title: MCP Integration
module: api
id: mcp-integration
order: 4
description: Model Context Protocol integration for AI tools
tags: [api, mcp, ai, tools, agents]
---

# MCP Integration

Complete guide to using Synap with the Model Context Protocol (MCP) for AI tools and agents.

## Overview

Synap provides native support for the **Model Context Protocol (MCP)**, enabling seamless integration with AI tools, agents, and LLM-based applications. This allows AI systems to use Synap as a context-aware data store and message broker.

## What is MCP?

The Model Context Protocol is a standardized protocol for AI tools to interact with data sources and services. It provides:

- **Resource Discovery**: AI tools can discover available data and operations
- **Tool Integration**: Expose Synap operations as callable tools for AI agents
- **Context Management**: Efficient storage and retrieval of conversation context
- **Real-time Updates**: Subscribe to data changes and events

## MCP Server Setup

### Endpoint

**MCP Endpoint:**
```
http://localhost:15500/mcp
```

### Connection

```json
{
  "jsonrpc": "2.0",
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {
      "name": "my-client",
      "version": "1.0.0"
    }
  },
  "id": 1
}
```

## Resources

Synap exposes the following MCP resources:

### Key-Value Resources

- `kv://{key}` - Access individual key-value pairs
- `kv://prefix/{prefix}*` - List keys by prefix
- `kv://namespace/{namespace}/*` - Access namespaced keys

### Queue Resources

- `queue://{queue_name}` - Access queue messages
- `queue://{queue_name}/pending` - View pending messages
- `queue://{queue_name}/dead-letter` - Access dead letter queue

### Stream Resources

- `stream://{room_id}` - Access event stream for a room
- `stream://{room_id}/history` - Retrieve historical events
- `stream://{room_id}/subscribers` - List active subscribers

### Pub/Sub Resources

- `pubsub://{topic}` - Subscribe to topic messages
- `pubsub://pattern/{pattern}` - Wildcard topic subscriptions

## Tools

Synap provides MCP-compatible tools for AI agents:

### Key-Value Tools

#### synap_kv_get

Retrieve a value from the key-value store.

```json
{
  "name": "synap_kv_get",
  "description": "Retrieve a value from the key-value store",
  "inputSchema": {
    "type": "object",
    "properties": {
      "key": {"type": "string", "description": "The key to retrieve"}
    },
    "required": ["key"]
  }
}
```

**Example:**
```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "synap_kv_get",
    "arguments": {
      "key": "user:1"
    }
  },
  "id": 1
}
```

#### synap_kv_set

Store a value in the key-value store.

```json
{
  "name": "synap_kv_set",
  "description": "Store a value in the key-value store",
  "inputSchema": {
    "type": "object",
    "properties": {
      "key": {"type": "string"},
      "value": {"type": "string"},
      "ttl": {"type": "integer", "description": "Time to live in seconds"}
    },
    "required": ["key", "value"]
  }
}
```

### Queue Tools

#### synap_queue_publish

Publish a message to a queue.

```json
{
  "name": "synap_queue_publish",
  "description": "Publish a message to a queue",
  "inputSchema": {
    "type": "object",
    "properties": {
      "queue": {"type": "string"},
      "message": {"type": "string"},
      "priority": {"type": "integer", "minimum": 0, "maximum": 9}
    },
    "required": ["queue", "message"]
  }
}
```

#### synap_queue_consume

Consume a message from a queue.

```json
{
  "name": "synap_queue_consume",
  "description": "Consume a message from a queue",
  "inputSchema": {
    "type": "object",
    "properties": {
      "queue": {"type": "string"},
      "consumer": {"type": "string"}
    },
    "required": ["queue", "consumer"]
  }
}
```

## Use Cases

### AI Context Storage

Store conversation context for AI agents:

```json
{
  "method": "tools/call",
  "params": {
    "name": "synap_kv_set",
    "arguments": {
      "key": "context:session:123",
      "value": "{\"conversation\": [...], \"user_id\": 123}",
      "ttl": 3600
    }
  }
}
```

### Task Queue for AI Processing

Queue tasks for AI processing:

```json
{
  "method": "tools/call",
  "params": {
    "name": "synap_queue_publish",
    "arguments": {
      "queue": "ai-tasks",
      "message": "{\"task\": \"summarize\", \"text\": \"...\"}",
      "priority": 7
    }
  }
}
```

### Event-Driven AI Workflows

Subscribe to events for AI processing:

```json
{
  "method": "resources/subscribe",
  "params": {
    "uri": "pubsub://events.user.*"
  }
}
```

## Integration Examples

### Claude Desktop

Add to Claude Desktop configuration:

```json
{
  "mcpServers": {
    "synap": {
      "command": "npx",
      "args": [
        "-y",
        "@modelcontextprotocol/server-synap",
        "http://localhost:15500"
      ]
    }
  }
}
```

### Cursor IDE

Configure in Cursor settings:

```json
{
  "mcp": {
    "servers": {
      "synap": {
        "url": "http://localhost:15500/mcp"
      }
    }
  }
}
```

## Best Practices

### Use Namespaced Keys

```json
{
  "key": "context:session:123",
  "value": "..."
}
```

### Set Appropriate TTL

```json
{
  "key": "context:session:123",
  "value": "...",
  "ttl": 3600  // 1 hour
}
```

### Monitor Resource Usage

Use MCP resources to monitor and manage Synap data:

```json
{
  "method": "resources/read",
  "params": {
    "uri": "kv://prefix/context:*"
  }
}
```

## Related Topics

- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [StreamableHTTP Protocol](./STREAMABLE_HTTP.md) - Protocol documentation
- [UMICP Protocol](./UMICP.md) - Universal Matrix Inter-Communication Protocol

