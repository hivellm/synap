---
title: API Quick Reference
module: api
id: api-quick-reference
order: 8
description: Quick reference for common API endpoints
tags: [api, quick-reference, cheatsheet, endpoints]
---

# API Quick Reference

Quick reference for common Synap API endpoints.

## Base URL

```
http://localhost:15500
```

## System

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/info` | Server information |
| GET | `/metrics` | Prometheus metrics |

## Key-Value Store

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/kv/set` | Set key-value |
| GET | `/kv/get/{key}` | Get value |
| DELETE | `/kv/del/{key}` | Delete key |
| GET | `/kv/exists/{key}` | Check existence |
| POST | `/kv/mset` | Multiple set |
| POST | `/kv/mget` | Multiple get |
| POST | `/kv/incr/{key}` | Increment |
| POST | `/kv/decr/{key}` | Decrement |
| POST | `/kv/expire/{key}` | Set expiration |
| GET | `/kv/ttl/{key}` | Get TTL |
| GET | `/kv/stats` | Statistics |

## Hash

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/hash/{key}/hset` | Set field |
| GET | `/hash/{key}/hget/{field}` | Get field |
| GET | `/hash/{key}/hgetall` | Get all fields |
| DELETE | `/hash/{key}/hdel/{field}` | Delete field |
| POST | `/hash/{key}/mset` | Multiple set |

## List

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/list/{key}/lpush` | Push left |
| POST | `/list/{key}/rpush` | Push right |
| POST | `/list/{key}/lpop` | Pop left |
| POST | `/list/{key}/rpop` | Pop right |
| GET | `/list/{key}/lrange` | Get range |

## Set

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/set/{key}/sadd` | Add member |
| DELETE | `/set/{key}/srem/{member}` | Remove member |
| GET | `/set/{key}/smembers` | Get all members |
| GET | `/set/{key}/sismember/{member}` | Check membership |

## Sorted Set

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/sortedset/{key}/zadd` | Add member |
| GET | `/sortedset/{key}/zrange` | Get range |
| GET | `/sortedset/{key}/zrank/{member}` | Get rank |
| GET | `/sortedset/{key}/zscore/{member}` | Get score |

## Queues

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/queue/{name}` | Create queue |
| POST | `/queue/{name}/publish` | Publish message |
| GET | `/queue/{name}/consume/{consumer}` | Consume message |
| POST | `/queue/{name}/ack` | Acknowledge |
| POST | `/queue/{name}/nack` | Negative acknowledge |
| GET | `/queue/{name}/stats` | Statistics |

## Streams

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/stream/{name}` | Create stream |
| POST | `/stream/{name}/publish` | Publish event |
| GET | `/stream/{name}/consume/{consumer}` | Consume events |
| GET | `/stream/{name}/stats` | Statistics |

## Pub/Sub

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/pubsub/{topic}/publish` | Publish to topic |
| WebSocket | `/pubsub/ws` | Subscribe to topics |

## Cluster

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/cluster/info` | Cluster information |
| GET | `/cluster/nodes` | List nodes |
| POST | `/cluster/nodes` | Add node |
| DELETE | `/cluster/nodes/{node_id}` | Remove node |
| GET | `/cluster/slots` | Slot assignments |
| POST | `/cluster/slots/assign` | Assign slots |

## Response Codes

| Code | Meaning |
|------|---------|
| 200 | Success |
| 201 | Created |
| 204 | No Content |
| 400 | Bad Request |
| 401 | Unauthorized |
| 404 | Not Found |
| 500 | Internal Server Error |
| 503 | Service Unavailable |

## Error Format

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

- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [Authentication](./AUTHENTICATION.md) - Authentication guide

