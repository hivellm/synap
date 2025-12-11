# Synap Tutorials

Practical step-by-step tutorials for common use cases.

---

## Table of Contents

1. [Tutorial 1: Build a Rate Limiter](#tutorial-1-build-a-rate-limiter)
2. [Tutorial 2: Distributed Task Queue](#tutorial-2-distributed-task-queue)
3. [Tutorial 3: Real-Time Chat Application](#tutorial-3-real-time-chat-application)
4. [Tutorial 4: Session Management](#tutorial-4-session-management)
5. [Tutorial 5: Event-Driven Microservices](#tutorial-5-event-driven-microservices)
6. [Tutorial 6: Caching Layer](#tutorial-6-caching-layer)
7. [Tutorial 7: Pub/Sub Notification System](#tutorial-7-pubsub-notification-system)
8. [Tutorial 8: Kafka-Style Data Pipeline](#tutorial-8-kafka-style-data-pipeline)

---

## Tutorial 1: Build a Rate Limiter

**Goal**: Implement API rate limiting using Synap KV store

**Time**: 15 minutes

### Implementation

```python
import time
import requests

class RateLimiter:
    def __init__(self, synap_url, requests_per_minute=60):
        self.synap = synap_url
        self.rpm = requests_per_minute
    
    def is_allowed(self, user_id: str) -> bool:
        """Check if user is within rate limit"""
        key = f"ratelimit:{user_id}"
        current_minute = int(time.time() / 60)
        
        # Get current count
        resp = requests.get(f"{self.synap}/kv/get/{key}:{current_minute}")
        count = int(resp.text.strip('"')) if resp.status_code == 200 and resp.text != 'null' else 0
        
        if count >= self.rpm:
            return False
        
        # Increment counter with 60s TTL
        requests.post(
            f"{self.synap}/kv/set",
            json={
                "key": f"{key}:{current_minute}",
                "value": str(count + 1),
                "ttl": 60
            }
        )
        return True

# Usage
limiter = RateLimiter("http://localhost:15500", requests_per_minute=100)

if limiter.is_allowed("user:123"):
    # Process request
    print("Request allowed")
else:
    # Return 429 Too Many Requests
    print("Rate limit exceeded")
```

### Testing

```bash
# Run 150 requests in 1 minute
for i in {1..150}; do
  python test_rate_limiter.py
done

# First 100: Allowed
# Next 50: Rate limited
```

---

## Tutorial 2: Distributed Task Queue

**Goal**: Build a scalable background job processor

**Time**: 20 minutes

### Setup

**1. Create Queue**:
```bash
curl -X POST http://localhost:15500/queue/background-jobs \
  -H "Content-Type: application/json" \
  -d '{
    "max_depth": 100000,
    "ack_deadline_secs": 300,
    "default_max_retries": 3
  }'
```

### Producer (Web App)

```python
import requests
import json

class JobProducer:
    def __init__(self, synap_url):
        self.synap = synap_url
    
    def submit_job(self, job_type: str, data: dict, priority: int = 5):
        """Submit background job"""
        payload = json.dumps({"type": job_type, "data": data}).encode()
        
        resp = requests.post(
            f"{self.synap}/queue/background-jobs/publish",
            json={
                "payload": list(payload),
                "priority": priority,
                "max_retries": 3
            }
        )
        
        return resp.json()

# Usage
producer = JobProducer("http://localhost:15500")

# Submit email job (high priority)
producer.submit_job("send_email", {
    "to": "user@example.com",
    "subject": "Welcome!",
    "body": "Thanks for signing up"
}, priority=9)

# Submit image processing (normal priority)
producer.submit_job("resize_image", {
    "image_id": "img_123",
    "sizes": [100, 200, 500]
}, priority=5)
```

### Consumer (Worker)

```python
import requests
import json
import time
from typing import Optional

class JobWorker:
    def __init__(self, synap_url, worker_id: str):
        self.synap = synap_url
        self.worker_id = worker_id
        self.running = True
    
    def consume_loop(self):
        """Main worker loop"""
        while self.running:
            job = self.consume_job()
            
            if job:
                self.process_job(job)
            else:
                time.sleep(1)  # No jobs, wait
    
    def consume_job(self) -> Optional[dict]:
        """Consume next job from queue"""
        resp = requests.get(
            f"{self.synap}/queue/background-jobs/consume/{self.worker_id}"
        )
        
        if resp.status_code == 200:
            return resp.json()
        return None
    
    def process_job(self, job: dict):
        """Process job and ACK/NACK"""
        message_id = job["message_id"]
        payload = bytes(job["payload"]).decode()
        job_data = json.loads(payload)
        
        try:
            # Process based on type
            if job_data["type"] == "send_email":
                self.send_email(job_data["data"])
            elif job_data["type"] == "resize_image":
                self.resize_image(job_data["data"])
            else:
                print(f"Unknown job type: {job_data['type']}")
                
            # ACK success
            self.ack(message_id)
            print(f"✓ Processed job {message_id}")
            
        except Exception as e:
            # NACK on error (will retry)
            print(f"✗ Error processing {message_id}: {e}")
            self.nack(message_id)
    
    def ack(self, message_id: str):
        requests.post(
            f"{self.synap}/queue/background-jobs/ack",
            json={"message_id": message_id}
        )
    
    def nack(self, message_id: str):
        requests.post(
            f"{self.synap}/queue/background-jobs/nack",
            json={"message_id": message_id}
        )
    
    def send_email(self, data: dict):
        # Implement email sending
        print(f"Sending email to {data['to']}")
        time.sleep(0.5)  # Simulate work
    
    def resize_image(self, data: dict):
        # Implement image resizing
        print(f"Resizing image {data['image_id']}")
        time.sleep(2)  # Simulate work

# Usage: Run multiple workers
worker = JobWorker("http://localhost:15500", "worker-1")
worker.consume_loop()
```

**Run Workers**:
```bash
# Terminal 1
python worker.py --id worker-1

# Terminal 2
python worker.py --id worker-2

# Terminal 3
python worker.py --id worker-3

# Scale to N workers as needed
```

---

## Tutorial 3: Real-Time Chat Application

**Goal**: Build a multi-room chat system with message history

**Time**: 30 minutes

### Backend (Node.js)

```javascript
const express = require('express');
const WebSocket = require('ws');
const fetch = require('node-fetch');

const app = express();
const SYNAP_URL = 'http://localhost:15500';

app.use(express.json());

// API: Create chat room
app.post('/rooms/:roomId', async (req, res) => {
  await fetch(`${SYNAP_URL}/stream/${req.params.roomId}`, {
    method: 'POST'
  });
  res.json({ success: true });
});

// API: Send message
app.post('/rooms/:roomId/messages', async (req, res) => {
  const { userId, message } = req.body;
  
  await fetch(`${SYNAP_URL}/stream/${req.params.roomId}/publish`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      event: 'message',
      data: JSON.stringify({
        userId,
        message,
        timestamp: Date.now()
      })
    })
  });
  
  res.json({ success: true });
});

// API: Get message history
app.get('/rooms/:roomId/history', async (req, res) => {
  const { from = 0, limit = 50 } = req.query;
  
  const resp = await fetch(
    `${SYNAP_URL}/stream/${req.params.roomId}/consume/history?from_offset=${from}&limit=${limit}`
  );
  
  const events = await resp.json();
  const messages = events.events.map(e => ({
    ...JSON.parse(e.data),
    offset: e.offset
  }));
  
  res.json({ messages });
});

app.listen(3000);
```

### Frontend (HTML + JavaScript)

```html
<!DOCTYPE html>
<html>
<head>
  <title>Synap Chat</title>
  <style>
    #messages { height: 400px; overflow-y: scroll; border: 1px solid #ccc; padding: 10px; }
    .message { margin: 5px 0; }
  </style>
</head>
<body>
  <h1>Chat Room: <span id="room-name"></span></h1>
  <div id="messages"></div>
  <input id="message-input" type="text" placeholder="Type message..." />
  <button onclick="sendMessage()">Send</button>

  <script>
    const roomId = 'general';
    const userId = 'user-' + Math.random().toString(36).substr(2, 9);
    
    // Connect to Synap stream
    const ws = new WebSocket(`ws://localhost:15500/stream/${roomId}/ws/${userId}?from_offset=0`);
    
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      const data = JSON.parse(msg.data);
      displayMessage(data);
    };
    
    function displayMessage(data) {
      const div = document.createElement('div');
      div.className = 'message';
      div.textContent = `[${data.userId}]: ${data.message}`;
      document.getElementById('messages').appendChild(div);
    }
    
    function sendMessage() {
      const input = document.getElementById('message-input');
      const message = input.value;
      
      fetch(`http://localhost:3000/rooms/${roomId}/messages`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ userId, message })
      });
      
      input.value = '';
    }
    
    // Load history on startup
    fetch(`http://localhost:3000/rooms/${roomId}/history`)
      .then(r => r.json())
      .then(data => {
        data.messages.forEach(displayMessage);
      });
  </script>
</body>
</html>
```

---

## Tutorial 4: Session Management

**Goal**: Replace Redis for session storage

**Time**: 15 minutes

### Express.js Session Store

```javascript
const express = require('express');
const session = require('express-session');
const fetch = require('node-fetch');

// Custom Synap session store
class SynapStore {
  constructor(synapUrl) {
    this.synap = synapUrl;
  }
  
  async get(sid, callback) {
    try {
      const resp = await fetch(`${this.synap}/kv/get/session:${sid}`);
      const data = await resp.text();
      
      if (data === 'null') {
        callback(null, null);
      } else {
        callback(null, JSON.parse(data.replace(/^"|"$/g, '')));
      }
    } catch (err) {
      callback(err);
    }
  }
  
  async set(sid, session, callback) {
    try {
      await fetch(`${this.synap}/kv/set`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          key: `session:${sid}`,
          value: JSON.stringify(session),
          ttl: 3600  // 1 hour
        })
      });
      callback(null);
    } catch (err) {
      callback(err);
    }
  }
  
  async destroy(sid, callback) {
    try {
      await fetch(`${this.synap}/kv/del/session:${sid}`, {
        method: 'DELETE'
      });
      callback(null);
    } catch (err) {
      callback(err);
    }
  }
}

// Express app
const app = express();

app.use(session({
  store: new SynapStore('http://localhost:15500'),
  secret: 'your-secret-key',
  resave: false,
  saveUninitialized: false,
  cookie: { maxAge: 3600000 }  // 1 hour
}));

app.get('/', (req, res) => {
  req.session.views = (req.session.views || 0) + 1;
  res.send(`Views: ${req.session.views}`);
});

app.listen(3000);
```

---

## Tutorial 5: Event-Driven Microservices

**Goal**: Build loosely-coupled services using Pub/Sub

**Time**: 30 minutes

### Architecture

```
Order Service → Pub/Sub → Email Service
             ↓          → Inventory Service
             ↓          → Analytics Service
```

### Order Service (Publisher)

```python
import requests

class OrderService:
    def __init__(self, synap_url):
        self.synap = synap_url
    
    def create_order(self, user_id: str, items: list, total: float):
        # Save order to database
        order_id = save_to_db(user_id, items, total)
        
        # Publish event
        self.publish_event("events.order.created", {
            "order_id": order_id,
            "user_id": user_id,
            "items": items,
            "total": total,
            "timestamp": time.time()
        })
        
        return order_id
    
    def publish_event(self, topic: str, data: dict):
        requests.post(
            f"{self.synap}/pubsub/{topic}/publish",
            json={"message": json.dumps(data)}
        )

# Usage
service = OrderService("http://localhost:15500")
service.create_order("user-123", ["item1", "item2"], 99.99)
```

### Email Service (Subscriber)

```python
import asyncio
import websockets
import json

async def email_service():
    uri = "ws://localhost:15500/pubsub/ws?topics=events.order.#"
    
    async with websockets.connect(uri) as ws:
        print("Email Service listening...")
        
        async for message in ws:
            event = json.loads(message)
            data = json.loads(event["message"])
            
            if event["topic"] == "events.order.created":
                send_order_confirmation(data["user_id"], data["order_id"])
            elif event["topic"] == "events.order.shipped":
                send_shipping_notification(data["user_id"], data["tracking"])

def send_order_confirmation(user_id, order_id):
    print(f"📧 Sending order confirmation to {user_id} for order {order_id}")
    # Implement email sending

asyncio.run(email_service())
```

### Inventory Service (Subscriber)

```python
import asyncio
import websockets
import json

async def inventory_service():
    uri = "ws://localhost:15500/pubsub/ws?topics=events.order.created"
    
    async with websockets.connect(uri) as ws:
        print("Inventory Service listening...")
        
        async for message in ws:
            event = json.loads(message)
            data = json.loads(event["message"])
            
            # Decrease inventory
            for item in data["items"]:
                decrease_stock(item)
            
            print(f"📦 Updated inventory for order {data['order_id']}")

asyncio.run(inventory_service())
```

---

## Tutorial 6: Caching Layer

**Goal**: Implement multi-tier caching for database queries

**Time**: 20 minutes

### Implementation

```python
import requests
import hashlib
import json
import time

class CacheLayer:
    def __init__(self, synap_url, ttl=300):
        self.synap = synap_url
        self.ttl = ttl  # 5 minutes default
    
    def cache_key(self, query: str, params: dict) -> str:
        """Generate cache key from query + params"""
        data = f"{query}:{json.dumps(params, sort_keys=True)}"
        return f"cache:query:{hashlib.md5(data.encode()).hexdigest()}"
    
    def get_cached(self, query: str, params: dict):
        """Get from cache"""
        key = self.cache_key(query, params)
        resp = requests.get(f"{self.synap}/kv/get/{key}")
        
        if resp.status_code == 200 and resp.text != 'null':
            print(f"💚 Cache HIT: {key}")
            return json.loads(resp.text.strip('"'))
        
        print(f"❌ Cache MISS: {key}")
        return None
    
    def set_cache(self, query: str, params: dict, result: any):
        """Store in cache"""
        key = self.cache_key(query, params)
        
        requests.post(
            f"{self.synap}/kv/set",
            json={
                "key": key,
                "value": json.dumps(result),
                "ttl": self.ttl
            }
        )
    
    def invalidate(self, pattern: str):
        """Invalidate cache by pattern"""
        # Use SCAN to find keys
        # DELETE matching keys
        pass

# Usage with database
cache = CacheLayer("http://localhost:15500", ttl=600)

def get_user(user_id: int):
    # Try cache first
    cached = cache.get_cached("SELECT * FROM users WHERE id = ?", {"id": user_id})
    if cached:
        return cached
    
    # Cache miss - query database
    result = db.query("SELECT * FROM users WHERE id = ?", user_id)
    
    # Store in cache
    cache.set_cache("SELECT * FROM users WHERE id = ?", {"id": user_id}, result)
    
    return result

# Usage
user = get_user(123)  # Database query
user = get_user(123)  # Cache hit!
```

### Cache Invalidation

```python
def update_user(user_id: int, data: dict):
    # Update database
    db.update("users", user_id, data)
    
    # Invalidate cache
    key = cache.cache_key("SELECT * FROM users WHERE id = ?", {"id": user_id})
    requests.delete(f"{cache.synap}/kv/del/{key}")
```

---

## Tutorial 7: Pub/Sub Notification System

**Goal**: System-wide notifications with multiple subscribers

**Time**: 25 minutes

### Notification Publisher

```python
class NotificationService:
    def __init__(self, synap_url):
        self.synap = synap_url
    
    def notify(self, category: str, event: str, data: dict):
        """Send notification to topic"""
        topic = f"notifications.{category}.{event}"
        
        requests.post(
            f"{self.synap}/pubsub/{topic}/publish",
            json={"message": json.dumps(data)}
        )

# Usage
notifier = NotificationService("http://localhost:15500")

# User signup
notifier.notify("user", "signup", {
    "user_id": "123",
    "email": "new@example.com"
})

# Payment received
notifier.notify("payment", "received", {
    "order_id": "456",
    "amount": 99.99
})

# System alert
notifier.notify("system", "error", {
    "service": "api",
    "error": "High error rate"
})
```

### Email Subscriber

```python
import asyncio
import websockets

async def email_subscriber():
    # Subscribe to all user notifications
    uri = "ws://localhost:15500/pubsub/ws?topics=notifications.user.#"
    
    async with websockets.connect(uri) as ws:
        async for message in ws:
            event = json.loads(message)
            data = json.loads(event["message"])
            
            if "signup" in event["topic"]:
                send_welcome_email(data["email"])
            elif "password_reset" in event["topic"]:
                send_reset_email(data["email"], data["token"])

asyncio.run(email_subscriber())
```

### Slack Subscriber

```python
async def slack_subscriber():
    # Subscribe to system alerts
    uri = "ws://localhost:15500/pubsub/ws?topics=notifications.system.#"
    
    async with websockets.connect(uri) as ws:
        async for message in ws:
            event = json.loads(message)
            data = json.loads(event["message"])
            
            send_slack_alert(
                channel="#alerts",
                message=f"🚨 {data['service']}: {data['error']}"
            )

asyncio.run(slack_subscriber())
```

### Analytics Subscriber

```python
async def analytics_subscriber():
    # Subscribe to ALL notifications
    uri = "ws://localhost:15500/pubsub/ws?topics=notifications.#"
    
    async with websockets.connect(uri) as ws:
        async for message in ws:
            event = json.loads(message)
            
            # Track in analytics system
            track_event(
                category=event["topic"],
                properties=json.loads(event["message"])
            )

asyncio.run(analytics_subscriber())
```

---

## Tutorial 8: Kafka-Style Data Pipeline

**Goal**: Process streaming data with consumer groups

**Time**: 30 minutes

### Create Partitioned Topic

```bash
curl -X POST http://localhost:15500/topics/user-events \
  -H "Content-Type: application/json" \
  -d '{
    "num_partitions": 4,
    "retention_policy": {
      "time": {
        "retention_secs": 86400
      }
    }
  }'
```

### Producer (Publish with Keys)

```python
import requests
import json

class EventProducer:
    def __init__(self, synap_url, topic):
        self.synap = synap_url
        self.topic = topic
    
    def publish(self, key: str, event_type: str, data: dict):
        """Publish to partition based on key"""
        requests.post(
            f"{self.synap}/topics/{self.topic}/publish",
            json={
                "key": key,  # Same key → same partition
                "data": json.dumps({
                    "type": event_type,
                    "data": data,
                    "timestamp": time.time()
                })
            }
        )

# Usage
producer = EventProducer("http://localhost:15500", "user-events")

# User actions (same user_id → same partition → ordered)
producer.publish("user:123", "page_view", {"page": "/home"})
producer.publish("user:123", "button_click", {"button": "signup"})
producer.publish("user:123", "form_submit", {"form": "registration"})

# Different users → different partitions (parallel processing)
producer.publish("user:456", "page_view", {"page": "/pricing"})
```

### Consumer Group

```python
class ConsumerGroup:
    def __init__(self, synap_url, topic, group_id, member_id):
        self.synap = synap_url
        self.topic = topic
        self.group_id = group_id
        self.member_id = member_id
        self.offset = 0
    
    def join_group(self):
        """Join consumer group"""
        resp = requests.post(
            f"{self.synap}/consumer-groups/{self.group_id}/join",
            json={
                "member_id": self.member_id,
                "topics": [self.topic]
            }
        )
        return resp.json()
    
    def get_assignment(self):
        """Get assigned partitions"""
        resp = requests.get(
            f"{self.synap}/consumer-groups/{self.group_id}/members/{self.member_id}/assignment"
        )
        return resp.json()["partitions"]
    
    def consume_loop(self):
        """Consume from assigned partitions"""
        self.join_group()
        
        while True:
            partitions = self.get_assignment()
            
            for partition_id in partitions:
                # Consume from partition
                resp = requests.post(
                    f"{self.synap}/topics/{self.topic}/partitions/{partition_id}/consume",
                    json={
                        "consumer_id": self.member_id,
                        "limit": 100
                    }
                )
                
                events = resp.json().get("events", [])
                
                for event in events:
                    self.process_event(event)
                    
                    # Commit offset
                    requests.post(
                        f"{self.synap}/consumer-groups/{self.group_id}/offsets/commit",
                        json={
                            "partition_id": partition_id,
                            "offset": event["offset"],
                            "consumer_id": self.member_id
                        }
                    )
            
            # Send heartbeat
            requests.post(
                f"{self.synap}/consumer-groups/{self.group_id}/members/{self.member_id}/heartbeat"
            )
            
            time.sleep(1)
    
    def process_event(self, event: dict):
        data = json.loads(event["data"])
        print(f"[{self.member_id}] Processing: {data['type']} - Partition {event['partition']}")

# Run multiple consumers in same group
# Partitions are automatically distributed

# Consumer 1
consumer1 = ConsumerGroup("http://localhost:15500", "user-events", "analytics", "worker-1")
consumer1.consume_loop()

# Consumer 2 (in another process)
# consumer2 = ConsumerGroup("http://localhost:15500", "user-events", "analytics", "worker-2")
# consumer2.consume_loop()
```

---

## More Tutorials

### Additional Resources

- **TypeScript SDK Examples**: See `sdks/typescript/examples/`
- **Python SDK Examples**: See `sdks/python/examples/`
- **Integration Examples**: See `docs/examples/`

### Community Tutorials

Submit your tutorials via GitHub Pull Requests!

---

## Next Steps

- Try the [User Guide](USER_GUIDE.md) for basic operations
- Read [Admin Guide](ADMIN_GUIDE.md) for deployment
- Check [API Reference](../api/REST_API.md) for complete API

---

**Have a tutorial idea?** Open an issue or PR on GitHub!

