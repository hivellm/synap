# Synap - High-Performance In-Memory Data Platform

[![Docker Pulls](https://img.shields.io/docker/pulls/hivehub/synap)](https://hub.docker.com/r/hivehub/synap)
[![Docker Image Size](https://img.shields.io/docker/image-size/hivehub/synap/latest)](https://hub.docker.com/r/hivehub/synap)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](https://github.com/hivellm/synap)

> **Unified data platform combining Redis, RabbitMQ, and Kafka in a single high-performance system**

Synap is a modern, production-ready in-memory data platform built in Rust that provides:

- 💾 **Key-Value Store** - Redis-compatible operations with radix tree storage
- 📨 **Message Queues** - RabbitMQ-style queues with ACK/NACK guarantees
- 📡 **Event Streams** - Kafka-style partitioned streams with consumer groups
- 🔔 **Pub/Sub** - Topic-based messaging with wildcard support
- 🔐 **Authentication** - Production-ready security with users, API keys, and permissions
- 💪 **Persistence** - WAL + Snapshots for durability
- 🔄 **Replication** - Master-slave replication for high availability

## 🚀 Quick Start

### Basic Usage

```bash
# Pull the image
docker pull hivehub/synap:latest

# Run a simple instance (all three protocols)
docker run -d \
  --name synap \
  -p 15500:15500 \
  -p 15501:15501 \
  -p 6379:6379 \
  -v synap-data:/data \
  hivehub/synap:latest

# Check health
curl http://localhost:15500/health
```

### With Docker Compose

```yaml
version: '3.8'

services:
  synap:
    image: hivehub/synap:latest
    container_name: synap-server
    ports:
      - "15500:15500"  # HTTP/REST API + StreamableHTTP
      - "15501:15501"  # SynapRPC binary protocol (synap://)
      - "6379:6379"    # RESP3 Redis-compatible (resp3://)
    volumes:
      - synap-data:/data
      - ./config.yml:/app/config.yml:ro
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:15500/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s

volumes:
  synap-data:
```

## 📋 Supported Tags

- `latest` - Latest stable release
- `0.11.0` - Specific version tag
- `0.11.x` - Version series tags

All images support multi-architecture builds:
- `linux/amd64` - Intel/AMD 64-bit
- `linux/arm64` - ARM 64-bit (Apple Silicon, ARM servers)

## 🔧 Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SYNAP_SERVER_ADDR` | Server bind address | `0.0.0.0:15500` |
| `SYNAP_REPLICATION_ADDR` | Replication bind address | `0.0.0.0:15501` |
| `SYNAP_DATA_DIR` | Data directory path | `/data` |
| `SYNAP_CONFIG_PATH` | Config file path | `/app/config.yml` |
| `SYNAP_AUTH_ENABLED` | Enable authentication | `false` |
| `SYNAP_AUTH_REQUIRE_AUTH` | Require authentication for all requests | `false` |
| `SYNAP_AUTH_ROOT_USERNAME` | Root user username | `root` |
| `SYNAP_AUTH_ROOT_PASSWORD` | Root user password | (must be set) |
| `SYNAP_AUTH_ROOT_ENABLED` | Enable root user | `true` |

### Volume Mounts

| Path | Description | Required |
|------|-------------|----------|
| `/data` | Data directory (WAL, snapshots) | Yes |
| `/app/config.yml` | Custom configuration file | No |

### Ports

| Port | Protocol | Description |
|------|----------|-------------|
| `15500` | HTTP/TCP | REST API, StreamableHTTP, WebSocket |
| `15501` | TCP | SynapRPC binary protocol (`synap://host:15501`) |
| `6379` | TCP | RESP3 Redis-compatible protocol (`resp3://host:6379`) |
| `15600` | TCP | Replication (master-slave, internal) |

## 📖 Usage Examples

### Connecting via Different Protocols

Synap exposes three client-facing protocols. Choose based on your use case:

```bash
# HTTP/REST — universal, any HTTP client
curl http://localhost:15500/kv/get/mykey

# SynapRPC — lowest latency, use Synap SDKs (Rust, TypeScript, Python, etc.)
# Rust SDK:
#   let client = SynapClient::new(SynapConfig::new("synap://localhost:15501"))?;

# RESP3 — Redis-compatible, any Redis client
redis-cli -p 6379 GET mykey
```

### Basic Key-Value Operations

```bash
# Set a key
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{"key": "user:1", "value": "John Doe", "ttl": 3600}'

# Get a key
curl http://localhost:15500/kv/get/user:1

# Delete a key
curl -X DELETE http://localhost:15500/kv/delete/user:1
```

### Queue Operations

```bash
# Publish a message
curl -X POST http://localhost:15500/queue/publish \
  -H "Content-Type: application/json" \
  -d '{
    "queue": "tasks",
    "message": "process-video",
    "priority": 1
  }'

# Consume a message
curl http://localhost:15500/queue/consume/tasks

# Acknowledge a message
curl -X POST http://localhost:15500/queue/ack \
  -H "Content-Type: application/json" \
  -d '{"queue": "tasks", "message_id": "msg-123"}'
```

### Event Stream Operations

```bash
# Publish an event
curl -X POST http://localhost:15500/stream/publish \
  -H "Content-Type: application/json" \
  -d '{
    "room": "chat-room-1",
    "event": "message",
    "data": {"text": "Hello, World!"}
  }'

# Consume events
curl http://localhost:15500/stream/consume/chat-room-1?offset=0&limit=10
```

### Pub/Sub Operations

```bash
# Publish to a topic
curl -X POST http://localhost:15500/pubsub/publish \
  -H "Content-Type: application/json" \
  -d '{
    "topic": "notifications.email",
    "message": "New order received"
  }'
```

## 🔐 Authentication Setup

### Enable Authentication

```bash
docker run -d \
  --name synap \
  -p 15500:15500 \
  -p 15501:15501 \
  -p 6379:6379 \
  -v synap-data:/data \
  -e SYNAP_AUTH_ENABLED=true \
  -e SYNAP_AUTH_REQUIRE_AUTH=true \
  -e SYNAP_AUTH_ROOT_USERNAME=admin \
  -e SYNAP_AUTH_ROOT_PASSWORD=SecurePassword123! \
  -e SYNAP_AUTH_ROOT_ENABLED=true \
  hivehub/synap:latest
```

### Using Authentication

```bash
# Basic Auth
curl -X POST http://localhost:15500/kv/set \
  -u admin:SecurePassword123! \
  -H "Content-Type: application/json" \
  -d '{"key": "test", "value": "data"}'

# API Key (Bearer Token)
curl -X POST http://localhost:15500/kv/set \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"key": "test", "value": "data"}'
```

## 🔄 Replication Setup

### Master Node

```yaml
version: '3.8'

services:
  synap-master:
    image: hivehub/synap:latest
    ports:
      - "15500:15500"  # HTTP/REST
      - "15501:15501"  # SynapRPC
      - "6379:6379"    # RESP3
      - "15600:15600"  # Replication (internal)
    volumes:
      - master-data:/data
      - ./config-master.yml:/app/config.yml:ro
    restart: unless-stopped

volumes:
  master-data:
```

### Replica Node

```yaml
version: '3.8'

services:
  synap-replica:
    image: hivehub/synap:latest
    ports:
      - "15510:15500"  # HTTP/REST
      - "15511:15501"  # SynapRPC
      - "6380:6379"    # RESP3
    volumes:
      - replica-data:/data
      - ./config-replica.yml:/app/config.yml:ro
    environment:
      - SYNAP_MASTER_ADDRESS=master-host:15600
    restart: unless-stopped
    depends_on:
      - synap-master

volumes:
  replica-data:
```

## 📊 Monitoring

### Health Check

```bash
# Check server health
curl http://localhost:15500/health

# Response: {"service":"synap","status":"healthy","version":"0.11.0"}
```

### Prometheus Metrics

```bash
# Expose metrics endpoint (if enabled in config)
curl http://localhost:15500/metrics
```

### Server Information

```bash
# Get server stats
curl http://localhost:15500/info

# Get KV store stats
curl http://localhost:15500/kv/stats

# Get queue stats
curl http://localhost:15500/queue/stats/queue-name
```

## 🛠️ Advanced Configuration

### Custom Configuration File

```bash
# Create custom config.yml
cat > config.yml <<EOF
server:
  host: "0.0.0.0"
  port: 15500

kv_store:
  max_memory_mb: 1024
  eviction_policy: "lru"

queue:
  enabled: true
  max_depth: 100000

# Binary protocols (enabled by default)
synap_rpc:
  enabled: true
  host: "0.0.0.0"
  port: 15501

resp3:
  enabled: true
  host: "0.0.0.0"
  port: 6379

persistence:
  enabled: true
  wal:
    enabled: true
    path: "/data/wal/synap.wal"
  snapshot:
    enabled: true
    directory: "/data/snapshots"
    interval_secs: 300
EOF

# Run with custom config
docker run -d \
  --name synap \
  -p 15500:15500 \
  -p 15501:15501 \
  -p 6379:6379 \
  -v synap-data:/data \
  -v $(pwd)/config.yml:/app/config.yml:ro \
  hivehub/synap:latest
```

### Production Deployment

```yaml
version: '3.8'

services:
  synap:
    image: hivehub/synap:0.11.0
    container_name: synap-production
    ports:
      - "15500:15500"  # HTTP/REST
      - "15501:15501"  # SynapRPC
      - "6379:6379"    # RESP3
    volumes:
      - /var/lib/synap/data:/data
      - /etc/synap/config.yml:/app/config.yml:ro
    environment:
      - SYNAP_SERVER_ADDR=0.0.0.0:15500
      - SYNAP_AUTH_ENABLED=true
      - SYNAP_AUTH_REQUIRE_AUTH=true
      - SYNAP_AUTH_ROOT_USERNAME=${SYNAP_ROOT_USER}
      - SYNAP_AUTH_ROOT_PASSWORD=${SYNAP_ROOT_PASS}
    restart: always
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:15500/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '1'
          memory: 1G
```

## 🔍 Troubleshooting

### Container Won't Start

```bash
# Check logs
docker logs synap

# Check if port is already in use
netstat -tuln | grep 15500

# Verify data directory permissions
ls -la /var/lib/synap/data
```

### Performance Issues

```bash
# Check memory usage
docker stats synap

# Check disk I/O
docker exec synap df -h /data

# Monitor slow queries (if enabled)
curl http://localhost:15500/slowlog
```

### Replication Issues

```bash
# Check replication status
curl http://localhost:15500/info | jq .replication

# Check replica connection
docker logs synap-replica | grep replication

# Verify network connectivity
docker exec synap-replica ping synap-master
```

## 📚 Additional Resources

- **GitHub Repository**: https://github.com/hivellm/synap
- **Documentation**: https://github.com/hivellm/synap/tree/main/docs
- **API Reference**: https://github.com/hivellm/synap/tree/main/docs/api
- **SDKs**: TypeScript, Python, Rust, PHP, C# available
- **Examples**: https://github.com/hivellm/synap/tree/main/docs/examples

## 🤝 Support

- **Issues**: https://github.com/hivellm/synap/issues
- **Discussions**: https://github.com/hivellm/synap/discussions

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](https://github.com/hivellm/synap/blob/main/LICENSE) file for details.

## 🏷️ Image Details

- **Base Image**: `alpine:3.19`
- **Size**: ~50MB (compressed)
- **Architecture**: Multi-arch (amd64, arm64)
- **User**: Non-root (`synap:synap`, UID 1000)
- **Health Check**: Built-in HTTP health endpoint

---

**Maintained by**: [HiveLLM](https://github.com/hivellm)

**Image Registry**: [Docker Hub](https://hub.docker.com/r/hivehub/synap)

