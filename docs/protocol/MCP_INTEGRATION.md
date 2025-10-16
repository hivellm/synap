# MCP (Model Context Protocol) Integration

## Overview

Synap provides native support for the **Model Context Protocol (MCP)**, enabling seamless integration with AI tools, agents, and LLM-based applications. This allows AI systems to use Synap as a context-aware data store and message broker.

## What is MCP?

The Model Context Protocol is a standardized protocol for AI tools to interact with data sources and services. It provides:

- **Resource Discovery**: AI tools can discover available data and operations
- **Tool Integration**: Expose Synap operations as callable tools for AI agents
- **Context Management**: Efficient storage and retrieval of conversation context
- **Real-time Updates**: Subscribe to data changes and events

## MCP Capabilities in Synap

### 1. Resources

Synap exposes the following MCP resources:

#### Key-Value Resources
- `kv://{key}` - Access individual key-value pairs
- `kv://prefix/{prefix}*` - List keys by prefix
- `kv://namespace/{namespace}/*` - Access namespaced keys

#### Queue Resources
- `queue://{queue_name}` - Access queue messages
- `queue://{queue_name}/pending` - View pending messages
- `queue://{queue_name}/dead-letter` - Access dead letter queue

#### Stream Resources
- `stream://{room_id}` - Access event stream for a room
- `stream://{room_id}/history` - Retrieve historical events
- `stream://{room_id}/subscribers` - List active subscribers

#### Pub/Sub Resources
- `pubsub://{topic}` - Subscribe to topic messages
- `pubsub://pattern/{pattern}` - Wildcard topic subscriptions

### 2. Tools

Synap provides MCP-compatible tools for AI agents:

#### Key-Value Tools
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

#### Queue Tools
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

```json
{
  "name": "synap_queue_consume",
  "description": "Consume a message from a queue",
  "inputSchema": {
    "type": "object",
    "properties": {
      "queue": {"type": "string"},
      "timeout": {"type": "integer", "description": "Timeout in seconds"}
    },
    "required": ["queue"]
  }
}
```

#### Stream Tools
```json
{
  "name": "synap_stream_publish",
  "description": "Publish an event to a stream room",
  "inputSchema": {
    "type": "object",
    "properties": {
      "room": {"type": "string"},
      "event": {"type": "string"},
      "data": {"type": "object"}
    },
    "required": ["room", "event", "data"]
  }
}
```

#### Pub/Sub Tools
```json
{
  "name": "synap_pubsub_publish",
  "description": "Publish a message to a topic",
  "inputSchema": {
    "type": "object",
    "properties": {
      "topic": {"type": "string"},
      "message": {"type": "string"}
    },
    "required": ["topic", "message"]
  }
}
```

### 3. Prompts

Synap can provide contextual prompts for AI agents:

#### System Prompts
- **data-retrieval**: Guide for efficient data retrieval patterns
- **queue-patterns**: Best practices for using queues
- **stream-usage**: How to work with event streams
- **pubsub-patterns**: Pub/sub messaging patterns

## Usage Examples

### Connecting to Synap via MCP

```typescript
import { McpClient } from '@modelcontextprotocol/sdk';

const client = new McpClient({
  transport: {
    type: 'stdio',
    command: 'synap-mcp-server',
    args: ['--host', 'localhost', '--port', '15500']
  }
});

await client.connect();
```

### Using Synap as Context Store

```typescript
// AI agent storing conversation context
await client.callTool('synap_kv_set', {
  key: `context:session:${sessionId}`,
  value: JSON.stringify(conversationHistory),
  ttl: 3600  // 1 hour
});

// Retrieving context
const context = await client.callTool('synap_kv_get', {
  key: `context:session:${sessionId}`
});
```

### Event-Driven AI Agent

```typescript
// Subscribe to events for real-time processing
await client.subscribe('stream://ai-events');

client.on('event', async (event) => {
  // Process event with AI
  const response = await processWithAI(event.data);
  
  // Publish response
  await client.callTool('synap_stream_publish', {
    room: 'ai-responses',
    event: 'response',
    data: response
  });
});
```

### Task Queue for AI Processing

```typescript
// AI agent consuming tasks from queue
while (true) {
  const task = await client.callTool('synap_queue_consume', {
    queue: 'ai-tasks',
    timeout: 30
  });
  
  if (task) {
    const result = await processTask(task.payload);
    
    // Acknowledge completion
    await client.callTool('synap_queue_ack', {
      queue: 'ai-tasks',
      message_id: task.id
    });
  }
}
```

## Configuration

Enable MCP support in Synap configuration:

```yaml
# config.yml
protocols:
  mcp:
    enabled: true
    port: 15501  # MCP-specific port
    features:
      - resources
      - tools
      - prompts
    
    # Resource limits
    limits:
      max_context_size: 1048576  # 1MB
      max_resources_per_request: 100
      rate_limit: 1000  # requests per minute
```

## Security

### Authentication
MCP connections require API key authentication:

```yaml
mcp:
  auth:
    enabled: true
    api_keys:
      - key: "mcp_key_abc123"
        name: "AI Agent 1"
        permissions: ["kv:read", "kv:write", "queue:consume"]
```

### Rate Limiting
Protect against abuse with rate limiting:

```yaml
mcp:
  rate_limiting:
    enabled: true
    requests_per_minute: 1000
    burst_size: 50
```

## Integration with AI Tools

### Cursor IDE
Synap can be used as a context provider in Cursor:

```json
{
  "mcpServers": {
    "synap": {
      "command": "synap-mcp-server",
      "args": ["--config", "/path/to/config.yml"]
    }
  }
}
```

### Claude Desktop
```json
{
  "mcpServers": {
    "synap": {
      "command": "synap-mcp-server",
      "args": ["--host", "localhost", "--port", "15500"]
    }
  }
}
```

### Custom AI Agents
```python
from mcp import Client

client = Client("synap-mcp-server", args=["--config", "config.yml"])
await client.connect()

# Use Synap tools in your AI agent
result = await client.call_tool("synap_kv_get", {"key": "user:123"})
```

## Benefits

### For AI Developers
- **Standardized Interface**: Use MCP standard instead of custom APIs
- **Context Management**: Efficient storage and retrieval of conversation context
- **Real-time Updates**: Subscribe to data changes and events
- **Tool Discovery**: Automatic discovery of available operations

### For AI Agents
- **Memory Persistence**: Store agent state and memory across sessions
- **Task Coordination**: Use queues for multi-agent task distribution
- **Event Broadcasting**: Publish and subscribe to system events
- **Structured Data**: Store and query structured data efficiently

## Performance Considerations

### Context Caching
- Use key-value store for frequently accessed context
- Set appropriate TTL for temporary context
- Use prefix queries for related context items

### Batch Operations
- Batch multiple operations in single MCP request
- Use pipeline for sequential operations
- Leverage streaming for large datasets

### Connection Pooling
- Reuse MCP connections across requests
- Configure connection pool size based on load
- Monitor connection health and reconnect as needed

## Monitoring

Track MCP usage with built-in metrics:

```
synap_mcp_requests_total{tool="synap_kv_get"}
synap_mcp_request_duration_seconds{tool="synap_kv_set"}
synap_mcp_active_connections
synap_mcp_errors_total{type="authentication"}
```

## Troubleshooting

### Connection Issues
```bash
# Test MCP connection
synap-mcp-client --test --host localhost --port 15501

# Check server logs
tail -f /var/log/synap/mcp.log
```

### Performance Issues
- Enable debug logging for MCP operations
- Monitor request latency and throughput
- Check resource limits and quotas
- Review authentication and rate limiting logs

## Future Enhancements

- **Streaming Resources**: Real-time streaming of large datasets
- **Vector Search**: Semantic search capabilities via MCP
- **Transaction Support**: Multi-operation transactions
- **Advanced Querying**: SQL-like queries for complex data retrieval

## See Also

- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [Synap REST API](../api/REST_API.md)
- [WebSocket Protocol](STREAMABLE_HTTP.md)
- [UMICP Integration](UMICP_INTEGRATION.md)

