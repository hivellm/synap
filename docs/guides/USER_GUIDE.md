# Synap User Guide

**Version**: 0.3.0  
**Last Updated**: October 22, 2025  
**Audience**: Developers and System Administrators

---

## Table of Contents

1. [Introduction](#introduction)
2. [Installation](#installation)
3. [Quick Start (5 Minutes)](#quick-start-5-minutes)
4. [Basic Operations](#basic-operations)
5. [Advanced Features](#advanced-features)
6. [Use Cases & Examples](#use-cases--examples)
7. [Troubleshooting](#troubleshooting)
8. [Best Practices](#best-practices)

---

## Introduction

### What is Synap?

Synap is a **high-performance in-memory data platform** that combines:

- 💾 **Key-Value Store** - Redis-like operations with radix tree storage
- 📨 **Message Queues** - RabbitMQ-style queues with ACK/NACK
- 📡 **Event Streams** - Kafka-style partitioned streams with consumer groups
- 🔔 **Pub/Sub** - Topic-based messaging with wildcard support

### Why Synap?

**All-in-One Solution**:
- Replace Redis + RabbitMQ + Kafka with a single system
- Consistent API across all data structures
- Built-in replication and persistence

**Performance**:
- 44K+ writes/sec (durable mode)
- 12M+ reads/sec
- Sub-microsecond latency (87ns for GET)
- 64-way sharding for parallelism

**Production-Ready**:
- Master-slave replication
- WAL + Snapshots for durability
- Prometheus metrics
- Docker + Kubernetes support

---

## Installation

### Option 1: Docker (Recommended)

**Single Instance**:
```bash
# Pull latest image
docker pull hivellm/synap:latest

# Run server
docker run -d \
  --name synap \
  -p 15500:15500 \
  -v synap-data:/data \
  hivellm/synap:latest

# Check status
curl http://localhost:15500/health
```

**With Docker Compose**:
```yaml
# docker-compose.yml
version: '3.8'
services:
  synap:
    image: hivellm/synap:latest
    ports:
      - "15500:15500"
    volumes:
      - ./data:/data
      - ./config.yml:/etc/synap/config.yml
    restart: unless-stopped
```

```bash
docker-compose up -d
```

### Option 2: Kubernetes (Helm)

```bash
# Add Helm repository
helm repo add synap https://hivellm.github.io/synap-charts
helm repo update

# Install
helm install my-synap synap/synap

# With custom values
helm install my-synap synap/synap -f values.yaml
```

**Production Setup** (Master + Replicas):
```bash
# Master
helm install synap-master synap/synap \
  --set replication.master.enabled=true \
  --set config.replication.role=master

# Replicas
helm install synap-replica synap/synap \
  --set replication.replica.enabled=true \
  --set replication.replica.replicaCount=2 \
  --set config.replication.role=replica
```

### Option 3: Binary Download

```bash
# Download from GitHub Releases
wget https://github.com/hivellm/synap/releases/download/v0.3.0/synap-linux-x64.tar.gz

# Extract
tar xzf synap-linux-x64.tar.gz
cd synap

# Run server
./synap-server --config config.example.yml

# In another terminal, use CLI
./synap-cli
```

### Option 4: Build from Source

```bash
# Prerequisites: Rust 1.85+ (Edition 2024)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default nightly

# Clone repository
git clone https://github.com/hivellm/synap.git
cd synap

# Build
cargo build --release

# Run
./target/release/synap-server --config config.yml
```

---

## Quick Start (5 Minutes)

### 1. Start Synap

```bash
# Using Docker
docker run -d -p 15500:15500 --name synap hivellm/synap:latest

# Or using binary
./synap-server --config config.yml
```

### 2. Verify Installation

```bash
# Health check
curl http://localhost:15500/health

# Expected output:
# {"status":"healthy","uptime_secs":5}
```

### 3. Your First Key-Value Operations

```bash
# Set a key
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"user:1","value":"John Doe","ttl":3600}'

# Get the key
curl http://localhost:15500/kv/get/user:1

# Output: "John Doe"

# Delete the key
curl -X DELETE http://localhost:15500/kv/del/user:1
```

### 4. Your First Queue Message

```bash
# Create queue
curl -X POST http://localhost:15500/queue/tasks \
  -H "Content-Type: application/json" \
  -d '{"max_depth":1000,"ack_deadline_secs":30}'

# Publish message
curl -X POST http://localhost:15500/queue/tasks/publish \
  -H "Content-Type: application/json" \
  -d '{"payload":[72,101,108,108,111],"priority":5}'

# Consume message
curl http://localhost:15500/queue/tasks/consume/worker-1

# Acknowledge message
curl -X POST http://localhost:15500/queue/tasks/ack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"<message_id_from_consume>"}'
```

### 5. Your First Stream Event

```bash
# Create stream room
curl -X POST http://localhost:15500/stream/chat-room-1

# Publish event
curl -X POST http://localhost:15500/stream/chat-room-1/publish \
  -H "Content-Type: application/json" \
  -d '{"event":"message","data":"Hello, World!"}'

# Consume events
curl "http://localhost:15500/stream/chat-room-1/consume/user-1?from_offset=0&limit=10"
```

**Congratulations! 🎉** You've completed the Quick Start!

---

## Basic Operations

### Key-Value Store

#### Setting Values

```bash
# Simple SET
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"mykey","value":"myvalue"}'

# SET with TTL (expires in 1 hour)
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"session:abc","value":"user-data","ttl":3600}'

# SET JSON value
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"user:1","value":"{\"name\":\"John\",\"age\":30}"}'
```

#### Getting Values

```bash
# GET single key
curl http://localhost:15500/kv/get/mykey

# Output: "myvalue"

# GET non-existent key
curl http://localhost:15500/kv/get/notfound

# Output: null
```

#### Deleting Values

```bash
# DELETE single key
curl -X DELETE http://localhost:15500/kv/del/mykey

# DELETE returns: {"deleted":true}
```

#### Statistics

```bash
# Get store statistics
curl http://localhost:15500/kv/stats

# Output:
# {
#   "total_keys": 42,
#   "memory_bytes": 8192,
#   "eviction_policy": "lru"
# }
```

### Message Queues

#### Queue Lifecycle

**1. Create Queue**:
```bash
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "max_depth": 10000,
    "ack_deadline_secs": 30,
    "default_max_retries": 3
  }'
```

**2. Publish Messages**:
```bash
# Publish with priority
curl -X POST http://localhost:15500/queue/jobs/publish \
  -H "Content-Type: application/json" \
  -d '{
    "payload": [72,101,108,108,111],
    "priority": 9,
    "max_retries": 3
  }'

# Priority: 0-9 (9 = highest)
```

**3. Consume Messages**:
```bash
# Consumer worker-1
curl http://localhost:15500/queue/jobs/consume/worker-1

# Output:
# {
#   "message_id": "abc-123-def",
#   "payload": [72,101,108,108,111],
#   "priority": 9,
#   "retry_count": 0
# }
```

**4. Acknowledge (ACK)**:
```bash
curl -X POST http://localhost:15500/queue/jobs/ack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"abc-123-def"}'

# Message removed from queue
```

**5. Negative Acknowledge (NACK)**:
```bash
curl -X POST http://localhost:15500/queue/jobs/nack \
  -H "Content-Type: application/json" \
  -d '{"message_id":"abc-123-def"}'

# Message goes back to queue (retry_count++)
```

#### Queue Statistics

```bash
curl http://localhost:15500/queue/jobs/stats

# Output:
# {
#   "pending": 42,
#   "in_flight": 5,
#   "dlq_count": 0,
#   "total_published": 1000
# }
```

### Event Streams

#### Stream Operations

**1. Create Room**:
```bash
curl -X POST http://localhost:15500/stream/notifications
```

**2. Publish Events**:
```bash
curl -X POST http://localhost:15500/stream/notifications/publish \
  -H "Content-Type: application/json" \
  -d '{
    "event": "user.signup",
    "data": "New user registered"
  }'
```

**3. Consume with Offset**:
```bash
# Start from beginning
curl "http://localhost:15500/stream/notifications/consume/user-1?from_offset=0&limit=100"

# Continue from last position
curl "http://localhost:15500/stream/notifications/consume/user-1?from_offset=42&limit=10"
```

**4. WebSocket (Real-time)**:
```javascript
// JavaScript example
const ws = new WebSocket('ws://localhost:15500/stream/notifications/ws/user-1?from_offset=0');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Event:', msg.event, 'Data:', msg.data);
};
```

#### Stream Statistics

```bash
curl http://localhost:15500/stream/notifications/stats

# Output:
# {
#   "message_count": 156,
#   "subscribers": 3,
#   "min_offset": 0,
#   "max_offset": 155
# }
```

### Pub/Sub

#### Publishing

```bash
# Publish to topic
curl -X POST http://localhost:15500/pubsub/notifications.email/publish \
  -H "Content-Type: application/json" \
  -d '{"message":"New order received"}'
```

#### Subscribing (WebSocket)

```javascript
// Subscribe to exact topic
const ws1 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email');

// Subscribe with wildcards
const ws2 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.*');

// Subscribe to multiple topics
const ws3 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.user.#,notifications.*');

ws1.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Received:', msg);
};
```

#### Wildcard Patterns

- `*` - Matches exactly one level
  - `notifications.*` matches `notifications.email`, `notifications.sms`
  
- `#` - Matches zero or more levels
  - `events.user.#` matches `events.user`, `events.user.login`, `events.user.login.success`

---

## Advanced Features

### Replication (Master-Slave)

#### Setup Master Node

**config-master.yml**:
```yaml
server:
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  role: "master"
  replica_listen_address: "0.0.0.0:15501"
  heartbeat_interval_ms: 1000
  max_lag_ms: 10000

persistence:
  enabled: true
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
  snapshot:
    enabled: true
    directory: "./data/snapshots"
```

```bash
synap-server --config config-master.yml
```

#### Setup Replica Node

**config-replica.yml**:
```yaml
server:
  host: "0.0.0.0"
  port: 15500

replication:
  enabled: true
  role: "replica"
  master_address: "master-host:15501"
  auto_reconnect: true
  reconnect_delay_ms: 5000

persistence:
  enabled: true
  wal:
    enabled: true
    path: "./data/wal/synap.wal"
```

```bash
synap-server --config config-replica.yml
```

#### Usage Pattern

```bash
# Write to MASTER
curl -X POST http://master-host:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"user:100","value":"Alice"}'

# Read from REPLICAS (load balancing)
curl http://replica1:15500/kv/get/user:100
curl http://replica2:15500/kv/get/user:100

# Eventually consistent (~5-10ms lag typical)
```

### Persistence & Durability

#### WAL (Write-Ahead Log)

```yaml
persistence:
  wal:
    enabled: true
    path: "/data/wal/synap.wal"
    fsync_mode: "periodic"  # or "always", "never"
    fsync_interval_ms: 10
```

**Modes**:
- `always` - Fsync every write (safest, slowest: ~1.7K ops/s)
- `periodic` - Fsync every 10ms (balanced: ~44K ops/s) ⭐ Recommended
- `never` - OS handles fsync (fastest: ~44K ops/s, risk on crash)

#### Snapshots

```yaml
persistence:
  snapshot:
    enabled: true
    directory: "/data/snapshots"
    interval_secs: 3600  # Every hour
    auto_snapshot: true
```

**Manual Snapshot**:
```bash
curl -X POST http://localhost:15500/snapshot
```

#### Recovery

Automatic on startup:
1. Load latest snapshot
2. Replay WAL from snapshot offset
3. Server ready (typically 1-10 seconds for 1M keys)

### Monitoring (Prometheus)

#### Metrics Endpoint

```bash
# Get all metrics
curl http://localhost:15500/metrics

# Example output:
# synap_kv_operations_total{operation="get",status="success"} 1234
# synap_kv_operation_duration_seconds{operation="get"} 0.000087
# synap_queue_depth{queue="jobs"} 42
# synap_process_memory_bytes{type="used"} 4294967296
```

#### Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'synap'
    static_configs:
      - targets: ['localhost:15500']
```

#### Grafana Dashboard

Import dashboard ID: `<coming-soon>` or use JSON in `docs/grafana/`

**Key Metrics to Monitor**:
- `synap_kv_operations_total` - KV throughput
- `synap_queue_depth` - Queue backlog
- `synap_replication_lag_operations` - Replication health
- `synap_process_memory_bytes` - Memory usage

### Authentication & Security

#### Enable Authentication

```yaml
# config.yml
authentication:
  enabled: true
  
users:
  - username: admin
    password_hash: "$2b$12$..."  # bcrypt hash
    role: admin
    
api_keys:
  - key: "sk_live_abc123..."
    name: "Production API"
    role: admin
    expires_days: 365
```

#### Basic Auth

```bash
curl -u admin:password http://localhost:15500/kv/stats
```

#### API Key

```bash
# Bearer token
curl -H "Authorization: Bearer sk_live_abc123..." \
  http://localhost:15500/kv/stats

# Query parameter
curl "http://localhost:15500/kv/stats?api_key=sk_live_abc123..."
```

---

## Use Cases & Examples

### Use Case 1: Session Store (Redis Replacement)

```bash
# Store session with 1 hour TTL
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{
    "key": "session:user-123",
    "value": "{\"user_id\":123,\"roles\":[\"admin\"]}",
    "ttl": 3600
  }'

# Retrieve session
SESSION=$(curl http://localhost:15500/kv/get/session:user-123)
echo $SESSION | jq .

# Extend TTL
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key":"session:user-123","value":"...","ttl":7200}'
```

### Use Case 2: Background Jobs (RabbitMQ Replacement)

**Producer** (Web Server):
```python
import requests

# Submit job
job = {
    "payload": list("process-video-123".encode()),
    "priority": 8,
    "max_retries": 3
}

response = requests.post(
    "http://localhost:15500/queue/video-processing/publish",
    json=job
)
print(f"Job submitted: {response.json()}")
```

**Consumer** (Worker):
```python
import requests
import time

worker_id = "worker-1"

while True:
    # Consume message
    resp = requests.get(
        f"http://localhost:15500/queue/video-processing/consume/{worker_id}"
    )
    
    if resp.status_code == 200:
        msg = resp.json()
        message_id = msg["message_id"]
        payload = bytes(msg["payload"]).decode()
        
        try:
            # Process job
            print(f"Processing: {payload}")
            process_video(payload)
            
            # ACK success
            requests.post(
                "http://localhost:15500/queue/video-processing/ack",
                json={"message_id": message_id}
            )
        except Exception as e:
            # NACK on error (will retry)
            requests.post(
                "http://localhost:15500/queue/video-processing/nack",
                json={"message_id": message_id}
            )
    else:
        time.sleep(1)  # No messages, wait
```

### Use Case 3: Real-Time Chat (Kafka Replacement)

**Backend** (WebSocket Server):
```javascript
const WebSocket = require('ws');

// Connect to Synap stream
const ws = new WebSocket('ws://localhost:15500/stream/chat-room-1/ws/server?from_offset=0');

ws.on('message', (data) => {
  const event = JSON.parse(data);
  
  // Broadcast to all connected clients
  broadcastToClients({
    type: event.event,
    data: event.data,
    offset: event.offset,
    timestamp: event.timestamp
  });
});

// When user sends message
function onUserMessage(userId, message) {
  fetch('http://localhost:15500/stream/chat-room-1/publish', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      event: 'message',
      data: JSON.stringify({ userId, message, timestamp: Date.now() })
    })
  });
}
```

**Frontend**:
```javascript
// Connect to chat room
const ws = new WebSocket('ws://localhost:15500/stream/chat-room-1/ws/user-123');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  const data = JSON.parse(msg.data);
  
  displayMessage(data.userId, data.message);
};

// Send message
function sendMessage(text) {
  fetch('http://localhost:15500/stream/chat-room-1/publish', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      event: 'message',
      data: JSON.stringify({ 
        userId: currentUserId, 
        message: text 
      })
    })
  });
}
```

### Use Case 4: Event Broadcasting (Pub/Sub)

**Publisher** (Order Service):
```bash
# Order created
curl -X POST http://localhost:15500/pubsub/events.order.created/publish \
  -H "Content-Type: application/json" \
  -d '{"message":"{\"order_id\":123,\"total\":99.99}"}'

# Payment received
curl -X POST http://localhost:15500/pubsub/events.payment.received/publish \
  -H "Content-Type: application/json" \
  -d '{"message":"{\"order_id\":123,\"amount\":99.99}"}'
```

**Subscriber 1** (Email Service):
```javascript
// Subscribe to all order events
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.order.#');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  sendEmail(msg.topic, msg.message);
};
```

**Subscriber 2** (Analytics):
```javascript
// Subscribe to all events
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.#');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  trackAnalytics(msg.topic, msg.message);
};
```

---

## Troubleshooting

### Server Won't Start

**Check port availability**:
```bash
# Linux/macOS
lsof -i :15500

# Windows
netstat -ano | findstr :15500
```

**Check configuration**:
```bash
# Validate config
synap-server --config config.yml --validate

# Check logs
docker logs synap
# or
tail -f server.log
```

### Connection Refused

**Verify server is running**:
```bash
curl http://localhost:15500/health
```

**Check firewall**:
```bash
# Linux
sudo ufw allow 15500/tcp

# Check listening
netstat -tlnp | grep 15500
```

### High Memory Usage

**Check statistics**:
```bash
curl http://localhost:15500/kv/stats

# Monitor with Prometheus
curl http://localhost:15500/metrics | grep memory
```

**Configure eviction**:
```yaml
kv_store:
  max_memory_mb: 4096
  eviction_policy: "lru"  # or "lfu"
```

### Replication Lag

**Check lag metrics**:
```bash
curl http://localhost:15500/metrics | grep replication_lag
```

**Common causes**:
1. Network latency between master/replica
2. Replica overloaded (too many reads)
3. Disk I/O bottleneck

**Solutions**:
- Increase `max_lag_ms` threshold
- Add more replicas
- Use faster network/disk

### Messages Stuck in Queue

**Check queue stats**:
```bash
curl http://localhost:15500/queue/jobs/stats
```

**Common issues**:
1. No consumers connected
2. All messages in DLQ (exceeded retries)
3. ACK deadline too short

**Solutions**:
```bash
# Purge queue (CAUTION!)
curl -X POST http://localhost:15500/queue/jobs/purge

# Or adjust ack_deadline
curl -X POST http://localhost:15500/queue/jobs \
  -H "Content-Type: application/json" \
  -d '{"ack_deadline_secs":60}'
```

---

## Best Practices

### 1. Use Appropriate Data Structures

| Use Case | Best Choice | Why |
|----------|-------------|-----|
| Cache | KV Store | TTL support, fast reads |
| Background Jobs | Queue | Retry logic, DLQ |
| Chat/Notifications | Streams | History, offset-based |
| System Events | Pub/Sub | Wildcard topics |

### 2. Key Naming Conventions

```bash
# Good: Namespaced, hierarchical
user:123:profile
session:abc-def-ghi
cache:product:456

# Bad: Flat, unclear
user123
mysession
temp
```

### 3. Queue Best Practices

- Set appropriate `ack_deadline` (30-60s typical)
- Use priorities (0-9) wisely
- Monitor DLQ regularly
- Implement idempotent workers

### 4. Replication Best Practices

- Always use persistence (WAL + Snapshots)
- Monitor replication lag
- Use replicas for read scaling
- Keep master/replica on same datacenter

### 5. Performance Tips

- Use batch operations when possible
- Enable L1 cache for hot data
- Use compression for large values
- Monitor Prometheus metrics

### 6. Security Best Practices

- Enable authentication in production
- Use TLS via reverse proxy (nginx/Caddy)
- Rotate API keys regularly
- Use RBAC for multi-tenant scenarios

---

## Next Steps

- 📖 Read [Admin Guide](ADMIN_GUIDE.md) for deployment and operations
- 🔧 See [API Reference](../api/REST_API.md) for complete API documentation
- 📊 Check [Performance Benchmarks](../BENCHMARK_RESULTS.md)
- 🐛 Report issues on [GitHub](https://github.com/hivellm/synap/issues)

---

## Getting Help

- **Documentation**: https://github.com/hivellm/synap
- **Issues**: https://github.com/hivellm/synap/issues
- **Discussions**: https://github.com/hivellm/synap/discussions

---

**Happy Coding with Synap! 🚀**

