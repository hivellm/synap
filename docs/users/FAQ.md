---
title: Frequently Asked Questions
module: users
id: faq
order: 100
description: Common questions and answers about Synap
tags: [faq, questions, answers, help]
---

# Frequently Asked Questions (FAQ)

Common questions and answers about Synap.

## General Questions

### What is Synap?

Synap is a high-performance in-memory data platform that combines:

- **Key-Value Store** - Redis-compatible operations
- **Message Queues** - RabbitMQ-style queues
- **Event Streams** - Kafka-style partitioned streams
- **Pub/Sub** - Topic-based messaging

All in a single, unified system built in Rust.

### Why use Synap instead of Redis/RabbitMQ/Kafka?

**Unified Platform:**
- Single system instead of managing three separate services
- Consistent API across all data structures
- Simplified deployment and operations

**Performance:**
- 44K+ writes/sec (durable mode)
- 12M+ reads/sec
- Sub-microsecond latency (87ns for GET)
- 64-way sharding for parallelism

**Production-Ready:**
- Built-in replication and persistence
- Prometheus metrics
- Docker and Kubernetes support
- Modern Rust implementation

### Is Synap production-ready?

Yes! Synap includes:

- ✅ Master-slave replication
- ✅ WAL + Snapshots for durability
- ✅ Authentication and authorization
- ✅ Prometheus metrics
- ✅ Health checks
- ✅ Docker and Kubernetes support
- ✅ Comprehensive test coverage

### What programming languages are supported?

Official SDKs:

- **Python** - `pip install synap-client`
- **TypeScript/JavaScript** - `npm install @hivehub/synap`
- **Rust** - `cargo add synap-client`

You can also use the REST API directly from any language.

### What's the license?

Apache 2.0 - Open source and free to use.

## Installation & Setup

### How do I install Synap?

**Docker (Recommended):**
```bash
docker pull hivellm/synap:latest
docker run -d --name synap -p 15500:15500 hivellm/synap:latest
```

**Binary:**
Download from [releases](https://github.com/hivellm/synap/releases)

**From Source:**
```bash
git clone https://github.com/hivellm/synap.git
cd synap
cargo build --release
```

See [Installation Guide](./getting-started/INSTALLATION.md) for details.

### What are the system requirements?

**Minimum:**
- 2 CPU cores
- 2GB RAM
- 10GB disk space

**Recommended:**
- 4+ CPU cores
- 8GB+ RAM
- SSD for persistence

### Can I run Synap on Windows?

Yes! Synap runs on:
- Linux (recommended)
- macOS
- Windows

See [Quick Start (Windows)](./getting-started/QUICK_START_WINDOWS.md).

### How do I configure Synap?

Create a `config.yml` file:

```yaml
server:
  host: 0.0.0.0
  port: 15500

persistence:
  enabled: true
  wal_path: ./data/wal
  snapshot_path: ./data/snapshots
```

See [Configuration Guide](./configuration/CONFIGURATION.md) for all options.

## Key-Value Store

### Is Synap compatible with Redis?

Synap provides Redis-compatible operations:

- ✅ SET, GET, DELETE
- ✅ Hash operations (HSET, HGET, etc.)
- ✅ List operations (LPUSH, RPOP, etc.)
- ✅ Set operations (SADD, SREM, etc.)
- ✅ Sorted Set operations (ZADD, ZRANGE, etc.)
- ✅ TTL support
- ✅ Atomic operations (INCR, DECR)

See [KV Store Guide](./kv-store/KV_STORE.md) for details.

### Can I migrate from Redis?

Yes! See [Migration Guide](./guides/MIGRATION.md) for step-by-step instructions.

### What data structures are supported?

- **Strings** - Basic key-value pairs
- **Hash** - Field-value maps
- **List** - Ordered collections
- **Set** - Unordered unique collections
- **Sorted Set** - Ordered unique collections with scores
- **HyperLogLog** - Cardinality estimation
- **Geospatial** - Geographic coordinates
- **Bitmap** - Bit-level operations

See [Data Structures Guide](./kv-store/DATA_STRUCTURES.md).

### How does TTL work?

TTL (Time To Live) automatically expires keys:

```bash
# Set key with 60 second TTL
POST /kv/set
{
  "key": "session:123",
  "value": "data",
  "ttl": 60
}
```

Keys are automatically deleted when TTL expires.

## Message Queues

### How do queues compare to RabbitMQ?

Synap queues provide:

- ✅ ACK/NACK support
- ✅ Priority queues
- ✅ Durable queues
- ✅ Multiple consumers
- ✅ Message persistence

See [Queues Guide](./queues/QUEUES.md).

### Can I migrate from RabbitMQ?

Yes! See [Migration Guide](./guides/MIGRATION.md).

### How do I handle message acknowledgments?

```python
# Consume message
message = queue.consume("my-queue", "consumer-1")

# Process message
process(message)

# Acknowledge
queue.ack("my-queue", "consumer-1", message.id)

# Or reject
queue.nack("my-queue", "consumer-1", message.id)
```

See [Consuming Messages](./queues/CONSUMING.md).

## Event Streams

### How do streams compare to Kafka?

Synap streams provide:

- ✅ Partitioned streams
- ✅ Consumer groups
- ✅ Offset management
- ✅ High throughput
- ✅ Durable storage

See [Streams Guide](./streams/STREAMS.md).

### Can I migrate from Kafka?

Yes! See [Migration Guide](./guides/MIGRATION.md).

### How do consumer groups work?

Consumer groups allow multiple consumers to share work:

```python
# Consumer 1
stream.consume("events", "group-1", "consumer-1")

# Consumer 2 (shares work)
stream.consume("events", "group-1", "consumer-2")
```

Each message is delivered to only one consumer in the group.

## Pub/Sub

### How do I subscribe to topics?

**WebSocket:**
```javascript
const ws = new WebSocket('ws://localhost:15500/pubsub/subscribe?topic=news.*');

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  console.log(message);
};
```

**SDK:**
```python
def on_message(topic, message):
    print(f"{topic}: {message}")

pubsub.subscribe("news.*", on_message)
```

See [Subscribing](./pubsub/SUBSCRIBING.md).

### Can I use wildcards?

Yes! Synap supports wildcard patterns:

- `news.*` - Matches `news.sports`, `news.tech`, etc.
- `*.sports` - Matches `news.sports`, `blog.sports`, etc.
- `*.*` - Matches all topics

See [Wildcards](./pubsub/WILDCARDS.md).

## Persistence & Replication

### How does persistence work?

Synap uses:

- **WAL (Write-Ahead Log)** - All writes are logged
- **Snapshots** - Periodic full state snapshots
- **Recovery** - Automatic recovery on startup

See [Persistence Guide](./guides/PERSISTENCE.md).

### How does replication work?

Master-replica replication:

- Master handles all writes
- Replicas receive updates asynchronously
- Automatic failover support

See [Replication Guide](./guides/REPLICATION.md).

### Can I use Synap without persistence?

Yes! Set `persistence.enabled: false` in config. Data will be in-memory only.

## Performance

### What's the performance?

**Benchmarks:**
- 44K+ writes/sec (durable mode)
- 12M+ reads/sec
- 87ns GET latency (p50)
- <1ms p95 latency

See [Performance Guide](./guides/PERFORMANCE.md).

### How do I optimize performance?

1. **Use sharding** - 64-way sharding enabled by default
2. **Tune persistence** - Adjust WAL and snapshot intervals
3. **Monitor metrics** - Use Prometheus metrics
4. **Scale horizontally** - Use replication

See [Performance Tuning](./configuration/PERFORMANCE_TUNING.md).

## Security

### How do I enable authentication?

```yaml
auth:
  enabled: true
  users:
    - username: admin
      password: secure-password
      roles: [admin]
```

See [Authentication Guide](./api/AUTHENTICATION.md).

### Can I use API keys?

Yes! Create API keys for programmatic access:

```bash
POST /auth/api-keys
{
  "name": "my-app",
  "permissions": ["kv:read", "queue:write"]
}
```

### Is TLS/SSL supported?

TLS support is planned. Currently, use a reverse proxy (Nginx, Caddy) for TLS termination.

See [Security Guide](./guides/SECURITY.md).

## API & Protocols

### What protocols are supported?

- **REST API** - Standard HTTP endpoints
- **StreamableHTTP** - Lightweight protocol over HTTP
- **MCP** - Model Context Protocol for AI tools
- **UMICP** - Universal Matrix Inter-Communication Protocol
- **WebSocket** - For Pub/Sub and streaming

See [API Reference](./api/API_REFERENCE.md).

### How do I use MCP?

MCP allows AI tools to interact with Synap:

```json
{
  "method": "tools/call",
  "params": {
    "name": "synap_kv_get",
    "arguments": {
      "key": "my-key"
    }
  }
}
```

See [MCP Integration](./api/MCP.md).

## Operations

### How do I monitor Synap?

**Prometheus Metrics:**
```bash
curl http://localhost:15500/metrics
```

**Health Check:**
```bash
curl http://localhost:15500/health
```

**Statistics:**
```bash
curl http://localhost:15500/info
```

See [Monitoring Guide](./operations/MONITORING.md).

### How do I backup data?

**Manual Backup:**
```bash
# Stop server
systemctl stop synap

# Copy data directory
cp -r /var/lib/synap/data /backup/synap-$(date +%Y%m%d)

# Start server
systemctl start synap
```

**Automated Backup:**
Use cron jobs or backup tools to copy the data directory.

See [Backup Guide](./operations/BACKUP.md).

### How do I view logs?

**Log Files:**
```bash
tail -f /var/log/synap/synap.log
```

**Docker:**
```bash
docker logs -f synap
```

See [Log Management](./operations/LOGS.md).

## Troubleshooting

### Server won't start

**Check:**
1. Port not in use: `lsof -i :15500`
2. Permissions: Ensure data directory is writable
3. Config file: Validate `config.yml` syntax

See [Troubleshooting Guide](./operations/TROUBLESHOOTING.md).

### High memory usage

**Solutions:**
1. Enable persistence to reduce memory pressure
2. Use TTL to expire old keys
3. Monitor with Prometheus metrics
4. Scale horizontally with replication

### Slow performance

**Check:**
1. Disk I/O - Use SSD for persistence
2. Network latency
3. Load - Monitor with metrics
4. Configuration - Tune persistence settings

## SDKs

### How do I install the Python SDK?

```bash
pip install synap-client
```

```python
from synap import SynapClient

client = SynapClient("http://localhost:15500")
client.kv.set("key", "value")
```

See [Python SDK Guide](./sdks/PYTHON.md).

### How do I install the TypeScript SDK?

```bash
npm install @hivehub/synap
```

```typescript
import { Synap } from '@hivehub/synap';

const client = new SynapClient('http://localhost:15500');
await client.kv.set('key', 'value');
```

See [TypeScript SDK Guide](./sdks/TYPESCRIPT.md).

### How do I install the Rust SDK?

```toml
[dependencies]
synap-client = "0.1"
```

```rust
use synap_client::SynapClient;

let client = SynapClient::new("http://localhost:15500").await?;
client.kv.set("key", "value").await?;
```

See [Rust SDK Guide](./sdks/RUST.md).

## Use Cases

### Can I use Synap for session storage?

Yes! Synap is perfect for session storage:

```python
# Store session
client.kv.set(f"session:{session_id}", session_data, ttl=3600)

# Retrieve session
session = client.kv.get(f"session:{session_id}")
```

See [Session Store Example](./use-cases/SESSION_STORE.md).

### Can I use Synap for background jobs?

Yes! Use queues for background jobs:

```python
# Producer
queue.publish("jobs", {"task": "process_image", "id": 123})

# Consumer
message = queue.consume("jobs", "worker-1")
process_job(message)
queue.ack("jobs", "worker-1", message.id)
```

See [Background Jobs Example](./use-cases/BACKGROUND_JOBS.md).

### Can I use Synap for real-time chat?

Yes! Use event streams for chat:

```python
# Publish message
stream.publish("chat:room-1", {"user": "alice", "message": "Hello"})

# Consume messages
for event in stream.consume("chat:room-1", "user-alice"):
    display_message(event)
```

See [Real-Time Chat Example](./use-cases/REAL_TIME_CHAT.md).

## Getting Help

### Where can I get help?

- **Documentation** - Check [User Guide](./README.md)
- **GitHub Issues** - [Report bugs](https://github.com/hivellm/synap/issues)
- **Troubleshooting** - See [Troubleshooting Guide](./operations/TROUBLESHOOTING.md)

### How do I report a bug?

1. Check [Troubleshooting Guide](./operations/TROUBLESHOOTING.md)
2. Search [GitHub Issues](https://github.com/hivellm/synap/issues)
3. Create new issue with:
   - Description
   - Steps to reproduce
   - Expected vs actual behavior
   - Logs and configuration

## Related Topics

- [Installation Guide](./getting-started/INSTALLATION.md) - Get started
- [Quick Start](./getting-started/QUICK_START.md) - First steps
- [Troubleshooting Guide](./operations/TROUBLESHOOTING.md) - Common problems
- [API Reference](./api/API_REFERENCE.md) - Complete API docs
- [Configuration Guide](./configuration/CONFIGURATION.md) - All options

