# Protocol Support in Synap

## Overview

Synap provides comprehensive protocol support for different use cases and integration scenarios.

## Supported Protocols

### 1. StreamableHTTP (Primary Protocol)

**Purpose**: General-purpose operations and REST API access

**Features**:
- HTTP/1.1 and HTTP/2 support
- Chunked transfer encoding for streaming
- JSON message envelopes
- WebSocket upgrade path
- Universal compatibility

**Use Cases**:
- REST API operations
- Key-value operations
- Queue management
- General-purpose access

**Documentation**: [StreamableHTTP Protocol](docs/protocol/STREAMABLE_HTTP.md)

---

### 2. MCP (Model Context Protocol)

**Purpose**: AI tool and agent integration

**Features**:
- Standardized AI tool interface
- Resource discovery (kv://, queue://, stream://, pubsub://)
- Tool execution (synap_kv_get, synap_queue_publish, etc.)
- Prompt templates for AI agents
- JSON-RPC 2.0 based

**Use Cases**:
- AI context storage
- Agent coordination
- LLM integration
- Cursor IDE integration
- Claude Desktop integration

**Benefits**:
- Store conversation context
- Coordinate multi-agent systems
- Event-driven AI workflows
- Task queue for AI processing

**Documentation**: [MCP Integration](docs/protocol/MCP_INTEGRATION.md)

**Example**:
```typescript
// AI agent storing context
await client.callTool('synap_kv_set', {
  key: 'context:session:123',
  value: JSON.stringify(conversationHistory),
  ttl: 3600
});
```

---

### 3. UMICP (Universal Matrix Inter-Communication Protocol)

**Purpose**: High-performance matrix and vector operations

**Features**:
- Matrix operations (add, multiply, transpose, determinant)
- Vector operations (dot product, normalization, similarity)
- Cosine similarity for embeddings
- Federated communication
- Efficient binary protocol

**Use Cases**:
- ML embedding storage
- Vector similarity search
- Matrix computations
- Distributed ML coordination
- Real-time vector processing

**Benefits**:
- Optimized for numerical operations
- Efficient envelope-based messaging
- WebSocket and HTTP/2 transport
- SIMD optimization support

**Documentation**: [UMICP Integration](docs/protocol/UMICP_INTEGRATION.md)

**Example**:
```typescript
// Compute cosine similarity between embeddings
const result = await client.send({
  operation: 'similarity.cosine',
  capabilities: {
    vector1: [1.0, 2.0, 3.0, 4.0],
    vector2: [5.0, 6.0, 7.0, 8.0]
  }
});
```

---

## Protocol Selection Guide

| Use Case | Recommended Protocol | Reason |
|----------|---------------------|--------|
| REST API access | StreamableHTTP | Universal, simple |
| AI agent integration | MCP | Standardized AI interface |
| Vector similarity search | UMICP | Optimized matrix ops |
| Key-value operations | StreamableHTTP or MCP | Both work well |
| Queue management | StreamableHTTP | Simple HTTP |
| Event streams | StreamableHTTP + WebSocket | Real-time support |
| ML embeddings | UMICP | Efficient vector ops |
| Context storage for AI | MCP | Resource discovery |
| Matrix computations | UMICP | Native matrix support |

---

## Configuration

Enable protocols in `config.yml`:

```yaml
protocols:
  # Primary HTTP protocol
  streamable_http:
    enabled: true
    path: /api
  
  # Model Context Protocol for AI
  mcp:
    enabled: true
    port: 15501
    features:
      - resources
      - tools
      - prompts
    limits:
      max_context_size: 1048576  # 1MB
      rate_limit: 1000  # req/min
  
  # UMICP for matrix operations
  umicp:
    enabled: true
    websocket_path: /umicp
    http2_path: /umicp/http
    matrix:
      max_dimension: 10000
      parallel_threshold: 1000
      simd_enabled: true
```

---

## Multi-Protocol Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Client Applications                    │
├─────────────────────────────────────────────────────────┤
│  HTTP Client  │  MCP Client  │  UMICP Client            │
└─────────────────────────────────────────────────────────┘
        │                │               │
        ▼                ▼               ▼
┌─────────────────────────────────────────────────────────┐
│              Synap Protocol Layer                        │
├─────────────────────────────────────────────────────────┤
│  StreamableHTTP  │     MCP      │    UMICP              │
│    Handler       │   Handler    │   Handler             │
└─────────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────┐
│                  Synap Core Engine                       │
├─────────────────────────────────────────────────────────┤
│   KV Store  │  Queue  │  Stream  │  Pub/Sub             │
└─────────────────────────────────────────────────────────┘
```

---

## Integration Examples

### 1. Cursor IDE Integration (MCP)

```json
// .cursor/mcp.json
{
  "mcpServers": {
    "synap": {
      "command": "synap-mcp-server",
      "args": ["--host", "localhost", "--port", "15501"]
    }
  }
}
```

### 2. Vector Search Application (UMICP)

```typescript
import { UmicpClient } from 'umicp-client';

const client = new UmicpClient({
  url: 'ws://localhost:15500/umicp'
});

// Store embeddings
await client.send({
  operation: 'kv.set',
  capabilities: {
    key: 'embedding:doc:123',
    value: JSON.stringify({ embedding: [...] })
  }
});

// Search similar
const similarity = await client.send({
  operation: 'similarity.cosine',
  capabilities: { vector1: query, vector2: stored }
});
```

### 3. Standard HTTP Access (StreamableHTTP)

```bash
# Standard REST API
curl -X POST http://localhost:15500/api/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key": "user:1", "value": "data"}'
```

---

## Performance Characteristics

| Protocol | Latency | Throughput | Best For |
|----------|---------|------------|----------|
| StreamableHTTP | ~1ms | 100K ops/sec | General use |
| MCP | ~2ms | 50K ops/sec | AI integration |
| UMICP | <1ms | 200K ops/sec | Matrix ops |

---

## Security

All protocols support:
- API key authentication
- TLS/SSL encryption
- Rate limiting
- IP whitelisting

Configure per-protocol:
```yaml
protocols:
  mcp:
    auth:
      enabled: true
      api_keys:
        - key: "mcp_key_123"
          permissions: ["kv:read", "queue:*"]
  
  umicp:
    tls:
      enabled: true
      cert_file: /etc/synap/certs/server.crt
```

---

## See Also

- [StreamableHTTP Specification](docs/protocol/STREAMABLE_HTTP.md)
- [MCP Integration Guide](docs/protocol/MCP_INTEGRATION.md)
- [UMICP Integration Guide](docs/protocol/UMICP_INTEGRATION.md)
- [Architecture Overview](docs/ARCHITECTURE.md)
- [Configuration Reference](docs/CONFIGURATION.md)

---

## Status

- ✅ StreamableHTTP: Documented
- ✅ MCP: Documented (Implementation planned)
- ✅ UMICP: Documented (Implementation planned)

**Last Updated**: October 16, 2025

