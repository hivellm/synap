---
title: Practical Examples
module: use-cases
id: practical-examples
order: 5
description: Real-world code examples and patterns
tags: [use-cases, examples, patterns, code]
---

# Practical Examples

Real-world code examples and patterns for using Synap.

## Rate Limiting

### Token Bucket Algorithm

```python
from synap_sdk import SynapClient
import time

client = SynapClient("http://localhost:15500")

def check_rate_limit(user_id, max_requests=100, window_secs=60):
    key = f"ratelimit:{user_id}"
    
    # Get current count
    count = client.kv.get(key)
    if count is None:
        count = 0
    
    count = int(count) if count else 0
    
    if count >= max_requests:
        return False  # Rate limited
    
    # Increment count
    client.kv.incr(key)
    client.kv.expire(key, window_secs)
    
    return True  # Allowed
```

## Distributed Lock

### Simple Lock

```python
from synap_sdk import SynapClient
import time
import uuid

client = SynapClient("http://localhost:15500")

def acquire_lock(lock_key, timeout_secs=10):
    lock_id = str(uuid.uuid4())
    lock_key = f"lock:{lock_key}"
    
    # Try to set lock (only if not exists)
    result = client.kv.set(lock_key, lock_id, ttl=timeout_secs)
    
    if result:
        return lock_id
    return None

def release_lock(lock_key, lock_id):
    lock_key = f"lock:{lock_key}"
    current_id = client.kv.get(lock_key)
    
    if current_id == lock_id:
        client.kv.delete(lock_key)
        return True
    return False
```

## Leader Election

### Simple Leader Election

```python
from synap_sdk import SynapClient
import time

client = SynapClient("http://localhost:15500")

def try_become_leader(leader_key, node_id, ttl=30):
    """Try to become leader"""
    key = f"leader:{leader_key}"
    
    # Try to set if not exists
    result = client.kv.set(key, node_id, ttl=ttl)
    
    if result:
        # We're the leader, renew periodically
        while True:
            time.sleep(ttl / 2)
            current = client.kv.get(key)
            if current == node_id:
                client.kv.expire(key, ttl)
            else:
                break  # Lost leadership
        return True
    
    return False  # Not the leader
```

## Cache-Aside Pattern

### Database with Cache

```python
from synap_sdk import SynapClient
import json

client = SynapClient("http://localhost:15500")
db = get_database_connection()

def get_user(user_id):
    # Try cache first
    cache_key = f"user:{user_id}"
    cached = client.kv.get(cache_key)
    
    if cached:
        return json.loads(cached)
    
    # Query database
    user = db.query("SELECT * FROM users WHERE id = ?", user_id)
    
    if user:
        # Cache result
        client.kv.set(cache_key, json.dumps(user), ttl=3600)
    
    return user

def update_user(user_id, data):
    # Update database
    db.update("users", user_id, data)
    
    # Invalidate cache
    client.kv.delete(f"user:{user_id}")
    
    # Or update cache
    # client.kv.set(f"user:{user_id}", json.dumps(data), ttl=3600)
```

## Event Sourcing

### Store Events

```python
from synap_sdk import SynapClient
import json
import time

client = SynapClient("http://localhost:15500")

def store_event(aggregate_id, event_type, event_data):
    """Store event in stream"""
    event = {
        "aggregate_id": aggregate_id,
        "type": event_type,
        "data": event_data,
        "timestamp": time.time()
    }
    
    client.stream.publish(
        f"events:{aggregate_id}",
        event_type,
        json.dumps(event),
        partition_key=aggregate_id
    )

def replay_events(aggregate_id):
    """Replay events for aggregate"""
    events = client.stream.consume(
        f"events:{aggregate_id}",
        f"replay-{aggregate_id}",
        from_offset=0,
        limit=1000
    )
    
    state = {}
    for event in events:
        event_data = json.loads(event.data)
        apply_event(state, event_data)
    
    return state
```

## Task Queue with Priority

### Priority Queue

```python
from synap_sdk import SynapClient
import json

client = SynapClient("http://localhost:15500")

def submit_task(task_type, task_data, priority=5):
    """Submit task with priority"""
    task = {
        "type": task_type,
        "data": task_data,
        "timestamp": time.time()
    }
    
    client.queue.publish(
        "tasks",
        json.dumps(task).encode(),
        priority=priority
    )

def process_tasks():
    """Process tasks from queue"""
    while True:
        message = client.queue.consume("tasks", "worker-1")
        
        if message:
            try:
                task = json.loads(message.payload.decode())
                execute_task(task)
                client.queue.ack("tasks", message.message_id)
            except Exception as e:
                client.queue.nack("tasks", message.message_id)
                log_error(e)
        else:
            time.sleep(1)
```

## Pub/Sub Event Bus

### Event Bus

```python
from synap_sdk import SynapClient
import json
import asyncio

client = SynapClient("http://localhost:15500")

def publish_event(event_type, event_data):
    """Publish event to bus"""
    topic = f"events.{event_type}"
    client.pubsub.publish(topic, json.dumps(event_data))

async def subscribe_to_events(event_pattern, handler):
    """Subscribe to events matching pattern"""
    async for message in client.pubsub.subscribe([event_pattern]):
        event_data = json.loads(message.message)
        handler(message.topic, event_data)

# Usage
async def handle_order_event(topic, data):
    if "order.created" in topic:
        send_confirmation_email(data)
    elif "order.paid" in topic:
        process_payment(data)

asyncio.run(subscribe_to_events("events.order.*", handle_order_event))
```

## Related Topics

- [Session Store](./SESSION_STORE.md) - Session storage example
- [Background Jobs](./BACKGROUND_JOBS.md) - Job queue example
- [Real-Time Chat](./REAL_TIME_CHAT.md) - Chat application example
- [Event Broadcasting](./EVENT_BROADCASTING.md) - Event-driven architecture

