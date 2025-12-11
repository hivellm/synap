---
title: API Documentation
module: api
id: api-index
order: 0
description: REST API, protocols, and integration documentation
tags: [api, rest, endpoints, integration]
---

# API Documentation

Complete REST API, protocol, and integration guides.

## API Reference

### [REST API Reference](./API_REFERENCE.md)

Complete reference for all API endpoints:

**REST API:**

- System endpoints (health, stats, info)
- Key-value operations (SET, GET, DELETE, etc.)
- Hash operations (HSET, HGET, HDEL, etc.)
- List operations (LPUSH, RPOP, LRANGE, etc.)
- Set operations (SADD, SREM, SINTER, etc.)
- Sorted Set operations (ZADD, ZRANGE, ZRANK, etc.)
- Queue management
- Stream operations
- Pub/Sub operations
- Cluster management

### [StreamableHTTP Protocol](./STREAMABLE_HTTP.md)

Protocol documentation:

- Message envelope format
- Command routing
- Error handling
- WebSocket upgrade

### [MCP Integration](./MCP.md)

Model Context Protocol integration:

- MCP server setup
- Tool discovery
- Resource URIs (kv://, queue://, stream://, pubsub://)
- Tool execution
- Complete tool reference

### [UMICP Protocol](./UMICP.md)

Universal Matrix Inter-Communication Protocol:

- Envelope-based communication
- Tool discovery endpoint
- High-performance streaming
- Binary protocol support

### [Cluster API](./CLUSTER.md)

Cluster management endpoints:

- Cluster information
- Node management
- Slot assignments
- Migration operations
- Failover management

### [Integration Guide](./INTEGRATION.md)

Integrating Synap with other systems:

- Web frameworks (FastAPI, Express, Axum)
- Databases (PostgreSQL, MongoDB)
- Message brokers (RabbitMQ, Kafka)
- LLMs (OpenAI, LangChain)
- Monitoring (Prometheus, Grafana, Datadog)
- CI/CD (GitHub Actions, GitLab CI)
- Reverse proxy (Nginx, Caddy, Traefik)

## Authentication

### [Authentication Guide](./AUTHENTICATION.md)

Security and access control:

- User authentication
- API keys
- RBAC (Role-Based Access Control)
- Rate limiting
- Token management

## Integration

### [Integration Guide](./INTEGRATION.md)

Integrating Synap with other systems:

- Web frameworks (FastAPI, Express, Axum)
- Databases (PostgreSQL, MongoDB)
- Message brokers (RabbitMQ, Kafka)
- Monitoring (Prometheus, Grafana, Datadog)
- CI/CD (GitHub Actions, GitLab CI)
- Reverse proxy (Nginx, Caddy, Traefik)

## Quick Reference

### [API Quick Reference](./QUICK_REFERENCE.md)

Quick reference cheatsheet:

- Common endpoints
- Response codes
- Error format
- Command reference

### Base URL

```
http://localhost:15500
```

### Common Endpoints

- `GET /health` - Health check
- `GET /info` - Server information
- `GET /kv/stats` - KV store statistics
- `POST /kv/set` - Set key-value
- `GET /kv/get/{key}` - Get value
- `DELETE /kv/del/{key}` - Delete key
- `POST /queue/{name}` - Create queue
- `POST /queue/{name}/publish` - Publish message
- `GET /queue/{name}/consume/{consumer}` - Consume message

## Response Format

**Success:**

```json
{
  "success": true,
  "data": { ... }
}
```

**Error:**

```json
{
  "success": false,
  "error": {
    "type": "error_type",
    "message": "Error message",
    "status_code": 400
  }
}
```

## Related Topics

- [KV Store Guide](../kv-store/KV_STORE.md) - Using KV store via API
- [Queues Guide](../queues/QUEUES.md) - Queue operations
- [Streams Guide](../streams/STREAMS.md) - Stream operations
- [SDKs Guide](../sdks/README.md) - Client SDKs that wrap the API

