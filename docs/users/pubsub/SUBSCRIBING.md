---
title: Subscribing to Topics
module: pubsub
id: pubsub-subscribing
order: 2
description: Subscribing to pub/sub topics via WebSocket
tags: [pubsub, subscribing, websocket, topics]
---

# Subscribing to Topics

How to subscribe to Synap pub/sub topics via WebSocket.

## Basic Subscription

### Single Topic

```javascript
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Topic:', msg.topic);
  console.log('Message:', msg.message);
};

ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};

ws.onclose = () => {
  console.log('WebSocket closed');
};
```

### Multiple Topics

```javascript
const topics = ['notifications.email', 'notifications.sms', 'events.order.created'];
const ws = new WebSocket(`ws://localhost:15500/pubsub/ws?topics=${topics.join(',')}`);

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Topic:', msg.topic);
  console.log('Message:', msg.message);
};
```

## Message Format

### Received Message

```json
{
  "topic": "notifications.email",
  "message": "New order received",
  "timestamp": 1234567890
}
```

### Parse JSON Messages

```javascript
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  
  // If message is JSON string, parse it
  let data;
  try {
    data = JSON.parse(msg.message);
  } catch (e) {
    data = msg.message;
  }
  
  console.log('Topic:', msg.topic);
  console.log('Data:', data);
};
```

## Connection Management

### Reconnect Logic

```javascript
let ws;
let reconnectDelay = 1000;
const maxReconnectDelay = 30000;

function connect() {
  ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email');
  
  ws.onopen = () => {
    console.log('Connected');
    reconnectDelay = 1000;  // Reset delay
  };
  
  ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    handleMessage(msg);
  };
  
  ws.onerror = (error) => {
    console.error('WebSocket error:', error);
  };
  
  ws.onclose = () => {
    console.log('Disconnected, reconnecting...');
    setTimeout(connect, reconnectDelay);
    reconnectDelay = Math.min(reconnectDelay * 2, maxReconnectDelay);
  };
}

connect();
```

### Heartbeat

```javascript
let heartbeatInterval;

ws.onopen = () => {
  // Send heartbeat every 30 seconds
  heartbeatInterval = setInterval(() => {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'ping' }));
    }
  }, 30000);
};

ws.onclose = () => {
  clearInterval(heartbeatInterval);
};
```

## Using SDKs

### Python

```python
import asyncio
from synap_sdk import SynapClient

async def subscribe():
    client = SynapClient("http://localhost:15500")
    
    async for message in client.pubsub.subscribe(["notifications.email"]):
        print(f"Topic: {message.topic}, Message: {message.message}")

asyncio.run(subscribe())
```

### TypeScript

```typescript
import { Synap } from "@hivehub/synap";

const client = new SynapClient("http://localhost:15500");

// Subscribe to topics
const subscription = client.pubsub.subscribe(["notifications.email"]);

for await (const message of subscription) {
  console.log(`Topic: ${message.topic}, Message: ${message.message}`);
}
```

### Rust

```rust
use synap_sdk::SynapClient;
use futures::StreamExt;

let client = SynapClient::new("http://localhost:15500")?;

// Subscribe to topics
let mut subscription = client.pubsub.subscribe(vec!["notifications.email".to_string()]).await?;

while let Some(message) = subscription.next().await {
    println!("Topic: {}, Message: {}", message.topic, message.message);
}
```

## Multiple Subscriptions

### Different Topics per Consumer

```javascript
// Email service
const emailWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email');

// SMS service
const smsWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.sms');

// Analytics service (all notifications)
const analyticsWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.*');
```

## Error Handling

### Handle Connection Errors

```javascript
ws.onerror = (error) => {
  console.error('WebSocket error:', error);
  // Implement retry logic
};

ws.onclose = (event) => {
  if (event.wasClean) {
    console.log('Connection closed cleanly');
  } else {
    console.log('Connection lost, reconnecting...');
    setTimeout(connect, 1000);
  }
};
```

### Handle Message Errors

```javascript
ws.onmessage = (event) => {
  try {
    const msg = JSON.parse(event.data);
    handleMessage(msg);
  } catch (error) {
    console.error('Error parsing message:', error);
  }
};
```

## Best Practices

### Use Wildcards for Flexibility

```javascript
// Subscribe to all notifications
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.*');

// Subscribe to all events
const ws2 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.#');
```

### Implement Reconnection

Always implement reconnection logic for production:

```javascript
function createSubscription(topics, onMessage) {
  let ws;
  let reconnectDelay = 1000;
  
  function connect() {
    ws = new WebSocket(`ws://localhost:15500/pubsub/ws?topics=${topics.join(',')}`);
    
    ws.onopen = () => {
      reconnectDelay = 1000;
    };
    
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      onMessage(msg);
    };
    
    ws.onclose = () => {
      setTimeout(connect, reconnectDelay);
      reconnectDelay = Math.min(reconnectDelay * 2, 30000);
    };
  }
  
  connect();
  return () => ws.close();
}
```

### Monitor Connection

```javascript
ws.onopen = () => {
  console.log('Subscribed to topics');
};

ws.onclose = () => {
  console.log('Subscription closed');
  // Notify monitoring system
};
```

## Related Topics

- [Publishing to Topics](./PUBLISHING.md) - Publishing messages
- [Wildcards](./WILDCARDS.md) - Pattern matching
- [Complete Pub/Sub Guide](./PUBSUB.md) - Comprehensive reference

