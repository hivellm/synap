---
title: Synap User Documentation
module: users
id: user-documentation-index
order: 0
description: Complete user documentation organized by topics
tags: [documentation, users, guide]
---

# Synap User Documentation

Welcome to the Synap user documentation! This section contains guides and tutorials organized by topic.

## 📚 Documentation by Topic

### 🚀 [Getting Started](./getting-started/)

Installation and quick start guides:

- [Installation Guide](./getting-started/INSTALLATION.md) - Quick installation and overview
- [Docker Installation](./getting-started/DOCKER.md) - Complete Docker deployment guide
- [Docker Authentication](./getting-started/DOCKER_AUTHENTICATION.md) - Docker registry authentication
- [Building from Source](./getting-started/BUILD_FROM_SOURCE.md) - Build from source code
- [Quick Start Guide](./getting-started/QUICK_START.md) - Get up and running in minutes
- [First Steps](./getting-started/FIRST_STEPS.md) - Complete guide after installation
- [Quick Start (Windows)](./getting-started/QUICK_START_WINDOWS.md) - Windows-specific guide

### 💾 [Key-Value Store](./kv-store/)

Redis-compatible key-value operations:

- [Basic Operations](./kv-store/BASIC.md) - SET, GET, DELETE operations
- [Advanced Operations](./kv-store/ADVANCED.md) - TTL, batch operations, atomic commands
- [Data Structures](./kv-store/DATA_STRUCTURES.md) - Hash, List, Set, Sorted Set, HyperLogLog
- [Complete KV Guide](./kv-store/KV_STORE.md) - Comprehensive reference

### 📨 [Message Queues](./queues/)

RabbitMQ-style message queues:

- [Creating Queues](./queues/CREATING.md) - How to create and configure queues
- [Publishing Messages](./queues/PUBLISHING.md) - Publishing with priorities
- [Consuming Messages](./queues/CONSUMING.md) - ACK/NACK patterns
- [Complete Queues Guide](./queues/QUEUES.md) - Comprehensive reference

### 📡 [Event Streams](./streams/)

Kafka-style partitioned streams:

- [Creating Streams](./streams/CREATING.md) - How to create streams
- [Publishing Events](./streams/PUBLISHING.md) - Event publishing
- [Consuming Events](./streams/CONSUMING.md) - Offset-based consumption
- [Complete Streams Guide](./streams/STREAMS.md) - Comprehensive reference

### 🔔 [Pub/Sub](./pubsub/)

Topic-based messaging:

- [Publishing](./pubsub/PUBLISHING.md) - Publishing to topics
- [Subscribing](./pubsub/SUBSCRIBING.md) - WebSocket subscriptions
- [Wildcards](./pubsub/WILDCARDS.md) - Pattern matching
- [Complete Pub/Sub Guide](./pubsub/PUBSUB.md) - Comprehensive reference

### 💻 [SDKs](./sdks/)

Client libraries and SDKs:

- [Python SDK](./sdks/PYTHON.md) - Complete Python SDK guide
- [TypeScript SDK](./sdks/TYPESCRIPT.md) - TypeScript/JavaScript SDK guide
- [Rust SDK](./sdks/RUST.md) - Complete Rust SDK guide
- [SDKs Overview](./sdks/SDKS.md) - Quick comparison and overview

### 🔌 [API Reference](./api/)

REST API and integration:

- [REST API Reference](./api/API_REFERENCE.md) - Complete API endpoint reference
- [Authentication](./api/AUTHENTICATION.md) - Users, API Keys, RBAC
- [StreamableHTTP Protocol](./api/STREAMABLE_HTTP.md) - Protocol documentation
- [MCP Integration](./api/MCP.md) - Model Context Protocol
- [UMICP Protocol](./api/UMICP.md) - Universal Matrix Inter-Communication Protocol
- [Cluster API](./api/CLUSTER.md) - Cluster management endpoints
- [Integration Guide](./api/INTEGRATION.md) - Integrating with other systems
- [API Quick Reference](./api/QUICK_REFERENCE.md) - Quick reference cheatsheet

### 🔧 [Configuration](./configuration/)

Complete configuration guides:

- [Configuration Overview](./configuration/CONFIGURATION.md) - Quick reference and overview
- [Server Configuration](./configuration/SERVER.md) - Network, ports, host binding
- [Logging Configuration](./configuration/LOGGING.md) - Log levels, filtering
- [Persistence Configuration](./configuration/PERSISTENCE.md) - WAL, snapshots, durability
- [Replication Configuration](./configuration/REPLICATION.md) - Master-replica setup
- [Performance Tuning](./configuration/PERFORMANCE_TUNING.md) - Optimization tips
- [Rate Limiting](./configuration/RATE_LIMITING.md) - Rate limiting configuration

### ⚙️ [Operations](./operations/)

Service management, monitoring, and troubleshooting:

- [Service Management](./operations/SERVICE_MANAGEMENT.md) - Linux systemd and Windows Service
- [Log Management](./operations/LOGS.md) - Viewing, filtering, and analyzing logs
- [Monitoring](./operations/MONITORING.md) - Health checks, Prometheus metrics, Grafana dashboards
- [Backup and Restore](./operations/BACKUP.md) - Backup procedures and restore operations
- [Troubleshooting](./operations/TROUBLESHOOTING.md) - Common problems and fixes
- [Slow Query Log](./operations/SLOWLOG.md) - Monitor slow queries

### 🎯 [Examples and Use Cases](./use-cases/)

Real-world examples and tutorials:

- [Session Store](./use-cases/SESSION_STORE.md) - Redis replacement for sessions
- [Background Jobs](./use-cases/BACKGROUND_JOBS.md) - RabbitMQ replacement for job queues
- [Real-Time Chat](./use-cases/REAL_TIME_CHAT.md) - Kafka replacement for event streams
- [Event Broadcasting](./use-cases/EVENT_BROADCASTING.md) - Pub/Sub patterns
- [Practical Examples](./use-cases/EXAMPLES.md) - Real-world code examples

### 🚀 [Advanced Guides](./guides/)

Advanced features and optimizations:

- [Replication](./guides/REPLICATION.md) - Master-replica replication
- [Persistence](./guides/PERSISTENCE.md) - WAL and snapshots
- [Cluster Mode](./guides/CLUSTER.md) - Cluster setup and management
- [Transactions](./guides/TRANSACTIONS.md) - MULTI/EXEC/WATCH
- [Lua Scripting](./guides/LUA_SCRIPTING.md) - Server-side scripting
- [Performance Optimization](./guides/PERFORMANCE.md) - Advanced optimization
- [Migration Guide](./guides/MIGRATION.md) - Migrating from Redis, RabbitMQ, Kafka
- [Security Guide](./guides/SECURITY.md) - Security best practices and hardening
- [GUI Dashboard](./guides/GUI_DASHBOARD.md) - Synap Desktop GUI for monitoring
- [Compression Guide](./guides/COMPRESSION.md) - Data compression and optimization
- [Benchmarking Guide](./guides/BENCHMARKING.md) - How to benchmark Synap

## 🚀 Quick Start

New to Synap? Start here:

1. **[Install Synap](./getting-started/INSTALLATION.md)** - Get Synap running
2. **[Quick Start Guide](./getting-started/QUICK_START.md)** - Create your first operations
3. **[Basic KV Operations](./kv-store/BASIC.md)** - Start with key-value store
4. **[Message Queues](./queues/CREATING.md)** - Learn about queues

## 📖 Additional Resources

- **[API Reference](../api/REST_API.md)** - Complete REST API documentation
- **[Architecture Guide](../ARCHITECTURE.md)** - System architecture details
- **[Performance Guide](../specs/PERFORMANCE.md)** - Performance optimization tips
- **[Technical Specifications](../specs/)** - Architecture and implementation details

## 💡 Need Help?

- **[FAQ](./FAQ.md)** - Frequently asked questions
- Check the [main README](../../README.md) for quick reference
- Review [troubleshooting guide](./operations/TROUBLESHOOTING.md) for common issues
- See [specifications](../specs/) for technical details

