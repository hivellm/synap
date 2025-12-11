---
title: Session Store
module: use-cases
id: session-store
order: 1
description: Redis replacement for session storage
tags: [use-cases, session, redis, cache]
---

# Session Store

Using Synap as a Redis replacement for session storage.

## Overview

Synap's key-value store is fully compatible with Redis session storage patterns, providing:
- Fast in-memory storage
- TTL (Time To Live) support
- High performance (12M+ reads/sec)
- Persistence options

## Basic Session Storage

### Store Session

```bash
# Store session with 1 hour TTL
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{
    "key": "session:user-123",
    "value": "{\"user_id\":123,\"roles\":[\"admin\"],\"expires_at\":1234567890}",
    "ttl": 3600
  }'
```

### Retrieve Session

```bash
curl http://localhost:15500/kv/get/session:user-123
```

### Extend Session

```bash
# Update with new TTL
curl -X POST http://localhost:15500/kv/set \
  -H "Content-Type: application/json" \
  -d '{
    "key": "session:user-123",
    "value": "{\"user_id\":123,\"roles\":[\"admin\"]}",
    "ttl": 7200
  }'
```

### Delete Session

```bash
curl -X DELETE http://localhost:15500/kv/del/session:user-123
```

## Integration Examples

### Express.js (Node.js)

```javascript
const express = require('express');
const axios = require('axios');
const app = express();

const SYNAP_URL = 'http://localhost:15500';

// Store session
async function storeSession(sessionId, sessionData) {
  await axios.post(`${SYNAP_URL}/kv/set`, {
    key: `session:${sessionId}`,
    value: JSON.stringify(sessionData),
    ttl: 3600  // 1 hour
  });
}

// Retrieve session
async function getSession(sessionId) {
  const response = await axios.get(`${SYNAP_URL}/kv/get/session:${sessionId}`);
  return response.data ? JSON.parse(response.data) : null;
}

// Middleware
app.use(async (req, res, next) => {
  const sessionId = req.cookies.sessionId;
  if (sessionId) {
    req.session = await getSession(sessionId) || {};
  } else {
    req.session = {};
  }
  next();
});

// Save session
app.use(async (req, res, next) => {
  res.on('finish', async () => {
    if (req.session && Object.keys(req.session).length > 0) {
      const sessionId = req.cookies.sessionId || generateSessionId();
      await storeSession(sessionId, req.session);
      res.cookie('sessionId', sessionId, { maxAge: 3600000 });
    }
  });
  next();
});
```

### FastAPI (Python)

```python
from fastapi import FastAPI, Request, Response
from synap_sdk import SynapClient
import json
import secrets

app = FastAPI()
client = SynapClient("http://localhost:15500")

@app.middleware("http")
async def session_middleware(request: Request, call_next):
    session_id = request.cookies.get("session_id")
    
    if session_id:
        session_data = await client.kv.get(f"session:{session_id}")
        if session_data:
            request.state.session = json.loads(session_data)
        else:
            request.state.session = {}
    else:
        request.state.session = {}
        session_id = secrets.token_urlsafe(32)
        request.state.session_id = session_id
    
    response = await call_next(request)
    
    # Save session
    if hasattr(request.state, 'session') and request.state.session:
        await client.kv.set(
            f"session:{session_id}",
            json.dumps(request.state.session),
            ttl=3600
        )
        response.set_cookie("session_id", session_id, max_age=3600)
    
    return response
```

### Axum (Rust)

```rust
use axum::{extract::Request, middleware::Next, response::Response};
use synap_sdk::SynapClient;
use serde_json;

async fn session_middleware(
    mut request: Request,
    next: Next,
) -> Response {
    let session_id = request.headers()
        .get("x-session-id")
        .and_then(|h| h.to_str().ok());
    
    let client = SynapClient::new("http://localhost:15500").unwrap();
    
    if let Some(sid) = session_id {
        if let Ok(Some(data)) = client.kv.get(&format!("session:{}", sid)).await {
            if let Ok(session) = serde_json::from_str::<serde_json::Value>(&data) {
                request.extensions_mut().insert(session);
            }
        }
    }
    
    let response = next.run(request).await;
    response
}
```

## Best Practices

### Session Key Naming

Use consistent, namespaced keys:
- `session:{session_id}` - User sessions
- `session:temp:{temp_id}` - Temporary sessions
- `session:api:{api_key}` - API key sessions

### TTL Management

```python
# Standard user session: 1 hour
ttl = 3600

# Remember me: 7 days
ttl = 604800

# API key: 30 days
ttl = 2592000

# Temporary: 5 minutes
ttl = 300
```

### Session Cleanup

Sessions automatically expire based on TTL. No manual cleanup needed.

### Monitoring

```bash
# Check session count
curl http://localhost:15500/kv/stats

# Monitor memory usage
curl http://localhost:15500/metrics | grep memory
```

## Performance

- **Read latency**: < 1ms (87ns typical)
- **Write latency**: < 1ms
- **Throughput**: 12M+ reads/sec, 44K+ writes/sec
- **Memory efficient**: Radix tree storage

## Migration from Redis

### Direct Replacement

Synap is Redis-compatible for session storage:

```python
# Redis
import redis
r = redis.Redis(host='localhost', port=6379)
r.setex('session:123', 3600, session_data)

# Synap (same pattern)
from synap_sdk import SynapClient
client = SynapClient("http://localhost:15500")
client.kv.set('session:123', session_data, ttl=3600)
```

### Key Differences

- Use HTTP REST API instead of Redis protocol
- Same TTL semantics
- Same key-value model
- Better performance in many cases

## Related Topics

- [Basic KV Operations](../kv-store/BASIC.md) - Basic operations
- [Advanced KV Operations](../kv-store/ADVANCED.md) - Advanced features
- [Configuration Guide](../configuration/CONFIGURATION.md) - Server configuration

