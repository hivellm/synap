---
title: Integration Guide
module: api
id: integration-guide
order: 7
description: Integrating Synap with other systems
tags: [api, integration, frameworks, databases, monitoring]
---

# Integration Guide

Complete guide to integrating Synap with other systems and frameworks.

## Web Frameworks

### FastAPI (Python)

```python
from fastapi import FastAPI
from synap_sdk import SynapClient

app = FastAPI()
client = SynapClient("http://localhost:15500")

@app.get("/user/{user_id}")
async def get_user(user_id: str):
    value = await client.kv.get(f"user:{user_id}")
    return {"user_id": user_id, "data": value}

@app.post("/user/{user_id}")
async def create_user(user_id: str, data: dict):
    await client.kv.set(f"user:{user_id}", str(data), ttl=3600)
    return {"success": True}
```

### Express.js (Node.js)

```javascript
const express = require('express');
const { Synap } = require('@hivehub/synap');

const app = express();
const client = new SynapClient('http://localhost:15500');

app.get('/user/:userId', async (req, res) => {
  const value = await client.kv.get(`user:${req.params.userId}`);
  res.json({ userId: req.params.userId, data: value });
});

app.post('/user/:userId', async (req, res) => {
  await client.kv.set(`user:${req.params.userId}`, JSON.stringify(req.body), { ttl: 3600 });
  res.json({ success: true });
});
```

### Axum (Rust)

```rust
use axum::{extract::Path, Json, Router};
use synap_sdk::SynapClient;
use serde_json::Value;

async fn get_user(Path(user_id): Path<String>) -> Json<Value> {
    let client = SynapClient::new("http://localhost:15500").unwrap();
    let value = client.kv.get(&format!("user:{}", user_id)).await.unwrap();
    Json(json!({ "user_id": user_id, "data": value }))
}

let app = Router::new()
    .route("/user/:user_id", get(get_user));
```

## Databases

### PostgreSQL Integration

Use Synap as cache layer:

```python
from synap_sdk import SynapClient
import psycopg2

client = SynapClient("http://localhost:15500")
db = psycopg2.connect("...")

def get_user(user_id):
    # Try cache first
    cached = client.kv.get(f"user:{user_id}")
    if cached:
        return json.loads(cached)
    
    # Query database
    cursor = db.cursor()
    cursor.execute("SELECT * FROM users WHERE id = %s", (user_id,))
    user = cursor.fetchone()
    
    # Cache result
    client.kv.set(f"user:{user_id}", json.dumps(user), ttl=3600)
    
    return user
```

### MongoDB Integration

```python
from synap_sdk import SynapClient
from pymongo import MongoClient

client = SynapClient("http://localhost:15500")
mongo = MongoClient("mongodb://localhost:27017")
db = mongo.mydb

def get_user(user_id):
    # Try cache
    cached = client.kv.get(f"user:{user_id}")
    if cached:
        return json.loads(cached)
    
    # Query MongoDB
    user = db.users.find_one({"_id": user_id})
    
    # Cache result
    client.kv.set(f"user:{user_id}", json.dumps(user), ttl=3600)
    
    return user
```

## Message Brokers

### RabbitMQ Replacement

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Producer
def publish_job(job_data):
    client.queue.publish("jobs", json.dumps(job_data).encode(), priority=5)

# Consumer
def consume_jobs():
    while True:
        message = client.queue.consume("jobs", "worker-1")
        if message:
            job_data = json.loads(message.payload.decode())
            process_job(job_data)
            client.queue.ack("jobs", message.message_id)
```

### Kafka Replacement

```python
from synap_sdk import SynapClient

client = SynapClient("http://localhost:15500")

# Producer
def publish_event(event_type, data):
    client.stream.publish("events", event_type, json.dumps(data))

# Consumer
def consume_events():
    last_offset = 0
    while True:
        events = client.stream.consume("events", "consumer-1", from_offset=last_offset, limit=10)
        for event in events:
            process_event(event)
            last_offset = event.offset + 1
```

## LLM Integration

### OpenAI Integration

```python
from synap_sdk import SynapClient
from openai import OpenAI

client = SynapClient("http://localhost:15500")
openai = OpenAI()

def chat_with_context(user_id, message):
    # Get conversation context
    context = client.kv.get(f"context:{user_id}")
    if context:
        context = json.loads(context)
    else:
        context = []
    
    # Add new message
    context.append({"role": "user", "content": message})
    
    # Call OpenAI
    response = openai.chat.completions.create(
        model="gpt-4",
        messages=context
    )
    
    # Add response to context
    context.append({"role": "assistant", "content": response.choices[0].message.content})
    
    # Save context
    client.kv.set(f"context:{user_id}", json.dumps(context), ttl=3600)
    
    return response.choices[0].message.content
```

### LangChain Integration

```python
from synap_sdk import SynapClient
from langchain.memory import ChatMessageHistory
from langchain.chat_models import ChatOpenAI

client = SynapClient("http://localhost:15500")
llm = ChatOpenAI()

def chat(user_id, message):
    # Get history from Synap
    history_data = client.kv.get(f"history:{user_id}")
    if history_data:
        history = ChatMessageHistory()
        history.messages = json.loads(history_data)
    else:
        history = ChatMessageHistory()
    
    # Add user message
    history.add_user_message(message)
    
    # Get response
    response = llm(history.messages)
    
    # Add AI response
    history.add_ai_message(response.content)
    
    # Save history
    client.kv.set(f"history:{user_id}", json.dumps([m.dict() for m in history.messages]), ttl=3600)
    
    return response.content
```

## Monitoring

### Prometheus Integration

Synap exposes Prometheus metrics at `/metrics`:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'synap'
    static_configs:
      - targets: ['localhost:15500']
```

### Grafana Dashboard

Import dashboard or create custom:

```json
{
  "dashboard": {
    "title": "Synap Metrics",
    "panels": [
      {
        "title": "KV Operations",
        "targets": [{
          "expr": "rate(synap_kv_operations_total[5m])"
        }]
      }
    ]
  }
}
```

### Datadog Integration

```python
from datadog import initialize, api

initialize(api_key='...', app_key='...')

# Send custom metrics
api.Metric.send(
    metric='synap.kv.operations',
    points=1000,
    tags=['operation:get']
)
```

## CI/CD

### GitHub Actions

```yaml
name: Test Synap Integration

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      synap:
        image: hivellm/synap:latest
        ports:
          - 15500:15500
    steps:
      - uses: actions/checkout@v2
      - name: Run tests
        run: |
          pytest tests/
```

### GitLab CI

```yaml
test:
  image: python:3.9
  services:
    - name: hivellm/synap:latest
      alias: synap
  script:
    - pip install -r requirements.txt
    - pytest tests/
```

## Reverse Proxy

### Nginx

```nginx
upstream synap {
    server localhost:15500;
}

server {
    listen 80;
    server_name synap.example.com;
    
    location / {
        proxy_pass http://synap;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Caddy

```caddy
synap.example.com {
    reverse_proxy localhost:15500
}
```

## Related Topics

- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [Authentication](./AUTHENTICATION.md) - Security and authentication
- [Configuration Guide](../configuration/CONFIGURATION.md) - Server configuration

