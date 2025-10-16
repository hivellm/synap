# UMICP (Universal Matrix Inter-Communication Protocol) Integration

## Overview

Synap provides native support for **UMICP (Universal Matrix Inter-Communication Protocol)**, enabling high-performance matrix operations, vector processing, and federated communication for AI/ML workloads.

## What is UMICP?

UMICP is a protocol designed for:

- **Matrix Operations**: Efficient matrix and vector operations
- **Federated Communication**: Peer-to-peer messaging and coordination
- **AI/ML Workflows**: Support for embeddings, similarity search, and neural network operations
- **Real-time Streaming**: Low-latency data streaming with multiplexing

## UMICP Capabilities in Synap

### 1. Matrix Operations

Synap exposes matrix and vector operations through UMICP:

#### Vector Operations
- `vector.add` - Vector addition
- `vector.subtract` - Vector subtraction
- `vector.multiply` - Element-wise multiplication
- `vector.dot` - Dot product
- `vector.normalize` - L2 normalization
- `vector.scale` - Scalar multiplication

#### Matrix Operations
- `matrix.add` - Matrix addition
- `matrix.multiply` - Matrix multiplication
- `matrix.transpose` - Matrix transposition
- `matrix.determinant` - Determinant calculation

#### Similarity Operations
- `similarity.cosine` - Cosine similarity between vectors
- `similarity.euclidean` - Euclidean distance
- `similarity.manhattan` - Manhattan distance

### 2. Envelope-Based Messaging

UMICP uses structured envelopes for all communications:

```json
{
  "type": "request",
  "operation": "data",
  "from": "client-001",
  "to": "synap-server",
  "message_id": "msg-12345",
  "capabilities": {
    "operation": "vector.dot",
    "vector1": [1.0, 2.0, 3.0],
    "vector2": [4.0, 5.0, 6.0]
  },
  "payload_hints": {
    "type": "vector",
    "encoding": "json",
    "size": 48
  }
}
```

### 3. Transport Support

UMICP in Synap supports multiple transports:

- **WebSocket**: Persistent bidirectional connections
- **HTTP/2**: Request-response with multiplexing
- **StreamableHTTP**: Compatible with existing Synap protocol

## Integration Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Client Applications                    │
├─────────────────────────────────────────────────────────┤
│              UMICP Client SDK                           │
│     (TypeScript / Python / Rust)                        │
└─────────────────────────────────────────────────────────┘
                        │
                 UMICP Protocol
                        │
┌─────────────────────────────────────────────────────────┐
│                    Synap Server                          │
├─────────────────────────────────────────────────────────┤
│  ┌────────────────────────────────────────────────┐     │
│  │        UMICP Protocol Handler                  │     │
│  │  - Envelope parsing                            │     │
│  │  - Message routing                             │     │
│  │  - Response streaming                          │     │
│  └────────────────────────────────────────────────┘     │
│                        │                                 │
│  ┌────────────────────┼────────────────────────┐        │
│  │                    │                        │        │
│  ▼                    ▼                        ▼        │
│ ┌─────────┐    ┌──────────┐          ┌──────────┐      │
│ │ Matrix  │    │ Key-Value│          │  Queue   │      │
│ │ Engine  │    │  Store   │          │  System  │      │
│ └─────────┘    └──────────┘          └──────────┘      │
└─────────────────────────────────────────────────────────┘
```

## Usage Examples

### WebSocket Connection

```typescript
import { UmicpClient } from 'umicp-client';

const client = new UmicpClient({
  url: 'ws://localhost:15500/umicp',
  protocol: 'websocket'
});

await client.connect();
```

### Vector Operations

```typescript
// Compute cosine similarity
const result = await client.send({
  operation: 'similarity.cosine',
  capabilities: {
    vector1: [1.0, 2.0, 3.0, 4.0],
    vector2: [5.0, 6.0, 7.0, 8.0]
  }
});

console.log('Similarity:', result.capabilities.similarity);
```

### Storing Embeddings

```typescript
// Store embedding vector in key-value store
await client.send({
  operation: 'kv.set',
  capabilities: {
    key: 'embedding:doc:123',
    value: JSON.stringify({
      text: 'Document text',
      embedding: [0.1, 0.2, 0.3, /* ... */]
    }),
    ttl: 3600
  }
});
```

### Batch Vector Processing

```typescript
// Process multiple vectors in parallel
const vectors = [
  [1.0, 2.0, 3.0],
  [4.0, 5.0, 6.0],
  [7.0, 8.0, 9.0]
];

const results = await Promise.all(
  vectors.map(v => client.send({
    operation: 'vector.normalize',
    capabilities: { vector: v }
  }))
);
```

### Real-time Vector Search

```typescript
// Subscribe to similarity search stream
await client.subscribe('stream://vector-search', async (event) => {
  const query = event.data.query_vector;
  
  // Compute similarity against stored vectors
  const similarities = await computeSimilarities(query);
  
  // Publish results
  await client.send({
    operation: 'stream.publish',
    capabilities: {
      room: 'search-results',
      event: 'results',
      data: { similarities }
    }
  });
});
```

## Configuration

Enable UMICP support in Synap configuration:

```yaml
# config.yml
protocols:
  umicp:
    enabled: true
    websocket:
      enabled: true
      path: /umicp
    http2:
      enabled: true
      path: /umicp/http
    
    # Matrix operation settings
    matrix:
      max_dimension: 10000
      parallel_threshold: 1000  # Use parallel processing for large matrices
      simd_enabled: true
    
    # Performance tuning
    performance:
      connection_pool_size: 100
      max_concurrent_operations: 1000
      operation_timeout: 30  # seconds
```

## Security

### Authentication

```yaml
umicp:
  auth:
    enabled: true
    mechanism: "api_key"  # or "jwt", "oauth2"
    api_keys:
      - key: "umicp_key_xyz789"
        name: "ML Service"
        permissions: ["matrix:*", "kv:read", "stream:publish"]
```

### Encryption

```yaml
umicp:
  tls:
    enabled: true
    cert_file: /etc/synap/certs/server.crt
    key_file: /etc/synap/certs/server.key
    client_auth: optional
```

## Use Cases

### 1. Embedding Storage and Retrieval

```typescript
// Store document embeddings
for (const doc of documents) {
  const embedding = await generateEmbedding(doc.text);
  
  await client.send({
    operation: 'kv.set',
    capabilities: {
      key: `embedding:${doc.id}`,
      value: JSON.stringify({
        doc_id: doc.id,
        embedding: embedding,
        metadata: doc.metadata
      })
    }
  });
}
```

### 2. Vector Similarity Search

```typescript
// Find similar documents
const queryEmbedding = await generateEmbedding(query);

// Get all stored embeddings (in production, use a proper vector index)
const keys = await client.send({
  operation: 'kv.keys',
  capabilities: { prefix: 'embedding:' }
});

const similarities = [];
for (const key of keys.capabilities.keys) {
  const doc = await client.send({
    operation: 'kv.get',
    capabilities: { key }
  });
  
  const docEmbedding = JSON.parse(doc.capabilities.value).embedding;
  
  const similarity = await client.send({
    operation: 'similarity.cosine',
    capabilities: {
      vector1: queryEmbedding,
      vector2: docEmbedding
    }
  });
  
  similarities.push({
    key,
    similarity: similarity.capabilities.similarity
  });
}

// Sort by similarity
similarities.sort((a, b) => b.similarity - a.similarity);
```

### 3. Real-time ML Model Coordination

```typescript
// Federated learning coordinator
await client.subscribe('stream://model-updates', async (event) => {
  const modelUpdate = event.data;
  
  // Aggregate model updates from multiple workers
  const aggregated = await aggregateModels(modelUpdate);
  
  // Publish aggregated model
  await client.send({
    operation: 'stream.publish',
    capabilities: {
      room: 'global-model',
      event: 'model-update',
      data: aggregated
    }
  });
});
```

### 4. Batch Matrix Operations

```typescript
// Process batch of matrix operations
const results = await client.sendBatch([
  {
    operation: 'matrix.multiply',
    capabilities: { matrix1: [[1, 2], [3, 4]], matrix2: [[5, 6], [7, 8]] }
  },
  {
    operation: 'matrix.transpose',
    capabilities: { matrix: [[1, 2, 3], [4, 5, 6]] }
  },
  {
    operation: 'matrix.determinant',
    capabilities: { matrix: [[1, 2], [3, 4]] }
  }
]);
```

## Performance Optimization

### Connection Pooling

```typescript
const client = new UmicpClient({
  url: 'ws://localhost:15500/umicp',
  pool: {
    size: 10,
    maxWaitTime: 5000
  }
});
```

### Parallel Processing

```yaml
umicp:
  matrix:
    parallel_threshold: 1000  # Use parallel for matrices > 1000 elements
    thread_pool_size: 8
    simd_enabled: true
```

### Caching

```typescript
// Cache frequently used vectors
await client.send({
  operation: 'kv.set',
  capabilities: {
    key: 'cache:embedding:common',
    value: JSON.stringify(embedding),
    ttl: 86400  // Cache for 24 hours
  }
});
```

## Monitoring

UMICP-specific metrics:

```
synap_umicp_connections_total
synap_umicp_operations_total{operation="vector.dot"}
synap_umicp_operation_duration_seconds{operation="matrix.multiply"}
synap_umicp_matrix_size_bytes
synap_umicp_errors_total{type="dimension_mismatch"}
```

## Error Handling

UMICP error responses:

```json
{
  "type": "error",
  "operation": "error",
  "message_id": "msg-12345",
  "error": {
    "code": "DIMENSION_MISMATCH",
    "message": "Vector dimensions don't match: 3 vs 4",
    "details": {
      "vector1_dim": 3,
      "vector2_dim": 4
    }
  }
}
```

Common error codes:
- `DIMENSION_MISMATCH` - Vector/matrix dimension mismatch
- `INVALID_OPERATION` - Unsupported operation
- `MATRIX_TOO_LARGE` - Exceeds maximum matrix size
- `TIMEOUT` - Operation timeout
- `INVALID_ENVELOPE` - Malformed UMICP envelope

## Compatibility

### UMICP Versions
- Synap supports UMICP v0.2.x
- Backwards compatible with v0.1.x clients
- Version negotiation in handshake

### Client SDKs
- **Rust**: `umicp-core` crate (built-in)
- **TypeScript**: `@hivellm/umicp-client`
- **Python**: `umicp-python`

## Future Enhancements

- **GPU Acceleration**: Offload matrix operations to GPU
- **Distributed Computing**: Distribute large matrix operations across nodes
- **Vector Index**: Native vector similarity search with indexing
- **Compression**: Compress large matrices for efficient transmission
- **Streaming**: Stream large matrices in chunks

## See Also

- [UMICP Specification](https://github.com/hivellm/umicp)
- [Synap Architecture](../ARCHITECTURE.md)
- [MCP Integration](MCP_INTEGRATION.md)
- [StreamableHTTP Protocol](STREAMABLE_HTTP.md)

