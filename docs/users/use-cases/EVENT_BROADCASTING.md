---
title: Event Broadcasting
module: use-cases
id: event-broadcasting
order: 4
description: Pub/Sub patterns for event-driven architecture
tags: [use-cases, pubsub, events, broadcasting, architecture]
---

# Event Broadcasting

Using Synap pub/sub for event-driven architecture and event broadcasting.

## Overview

Synap pub/sub provides:
- Topic-based messaging
- Wildcard subscriptions
- WebSocket support
- Fire-and-forget delivery

## Basic Pattern

### Publisher (Event Source)

```python
from synap_sdk import SynapClient
import json

client = SynapClient("http://localhost:15500")

def publish_order_created(order_id, user_id, total):
    """Publish order created event"""
    event_data = json.dumps({
        "order_id": order_id,
        "user_id": user_id,
        "total": total,
        "timestamp": time.time()
    })
    
    client.pubsub.publish("events.order.created", event_data)
    print(f"Order created event published: {order_id}")

def publish_payment_received(order_id, amount):
    """Publish payment received event"""
    event_data = json.dumps({
        "order_id": order_id,
        "amount": amount,
        "timestamp": time.time()
    })
    
    client.pubsub.publish("events.payment.received", event_data)
    print(f"Payment received event published: {order_id}")
```

### Subscriber (Event Handler)

```javascript
// Email service - subscribes to order events
const emailWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.order.*');

emailWs.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  const data = JSON.parse(msg.message);
  
  if (msg.topic === 'events.order.created') {
    sendOrderConfirmationEmail(data.user_id, data.order_id);
  } else if (msg.topic === 'events.order.paid') {
    sendPaymentConfirmationEmail(data.user_id, data.order_id);
  }
};

// Analytics service - subscribes to all events
const analyticsWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.#');

analyticsWs.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  const data = JSON.parse(msg.message);
  
  // Track event in analytics
  trackEvent(msg.topic, data);
};
```

## E-Commerce Example

### Order Service (Publisher)

```python
from synap_sdk import SynapClient
import json

client = SynapClient("http://localhost:15500")

class OrderService:
    def create_order(self, user_id, items, total):
        # Create order in database
        order_id = self.save_order(user_id, items, total)
        
        # Publish events
        self.publish_order_created(order_id, user_id, total)
        
        return order_id
    
    def process_payment(self, order_id, payment_data):
        # Process payment
        result = self.charge_payment(payment_data)
        
        if result.success:
            # Publish payment event
            self.publish_payment_received(order_id, result.amount)
            self.publish_order_paid(order_id)
        
        return result
    
    def publish_order_created(self, order_id, user_id, total):
        event_data = json.dumps({
            "order_id": order_id,
            "user_id": user_id,
            "total": total
        })
        client.pubsub.publish("events.order.created", event_data)
    
    def publish_payment_received(self, order_id, amount):
        event_data = json.dumps({
            "order_id": order_id,
            "amount": amount
        })
        client.pubsub.publish("events.payment.received", event_data)
    
    def publish_order_paid(self, order_id):
        event_data = json.dumps({"order_id": order_id})
        client.pubsub.publish("events.order.paid", event_data)
```

### Email Service (Subscriber)

```python
import asyncio
from synap_sdk import SynapClient
import json

client = SynapClient("http://localhost:15500")

async def email_service():
    """Email service subscribes to order events"""
    async for message in client.pubsub.subscribe(["events.order.*"]):
        data = json.loads(message.message)
        
        if message.topic == "events.order.created":
            send_order_confirmation_email(data["user_id"], data["order_id"])
        elif message.topic == "events.order.paid":
            send_payment_confirmation_email(data["user_id"], data["order_id"])

asyncio.run(email_service())
```

### Inventory Service (Subscriber)

```python
async def inventory_service():
    """Inventory service subscribes to order events"""
    async for message in client.pubsub.subscribe(["events.order.created"]):
        data = json.loads(message.message)
        
        # Update inventory
        update_inventory(data["order_id"], data["items"])
```

### Analytics Service (Subscriber)

```python
async def analytics_service():
    """Analytics service subscribes to all events"""
    async for message in client.pubsub.subscribe(["events.#"]):
        data = json.loads(message.message)
        
        # Track event
        track_event(message.topic, data)
```

## Notification System

### Notification Publisher

```python
def send_notification(user_id, notification_type, message):
    """Send notification to user"""
    event_data = json.dumps({
        "user_id": user_id,
        "type": notification_type,
        "message": message
    })
    
    # Publish to notification topic
    client.pubsub.publish(f"notifications.{notification_type}", event_data)
```

### Notification Subscribers

```javascript
// Email notification service
const emailWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.email');

emailWs.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  const data = JSON.parse(msg.message);
  sendEmail(data.user_id, data.message);
};

// SMS notification service
const smsWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.sms');

smsWs.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  const data = JSON.parse(msg.message);
  sendSMS(data.user_id, data.message);
};

// Push notification service
const pushWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=notifications.push');

pushWs.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  const data = JSON.parse(msg.message);
  sendPushNotification(data.user_id, data.message);
};
```

## System Events

### System Event Publisher

```python
def publish_system_event(event_type, severity, message):
    """Publish system event"""
    event_data = json.dumps({
        "type": event_type,
        "severity": severity,
        "message": message,
        "timestamp": time.time()
    })
    
    client.pubsub.publish(f"system.{event_type}", event_data)
```

### System Event Subscribers

```javascript
// Monitoring service - all system events
const monitoringWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=system.#');

monitoringWs.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  const data = JSON.parse(msg.message);
  
  // Log to monitoring system
  logToMonitoring(msg.topic, data);
  
  // Alert on critical events
  if (data.severity === 'critical') {
    sendAlert(msg.topic, data);
  }
};

// Alerting service - only alerts
const alertWs = new WebSocket('ws://localhost:15500/pubsub/ws?topics=system.alert.*');

alertWs.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  const data = JSON.parse(msg.message);
  sendAlert(data.message);
};
```

## Best Practices

### Use Hierarchical Topics

Organize topics hierarchically:

```
events.order.created
events.order.paid
events.order.shipped
events.user.signup
events.user.login
```

### Use Wildcards for Flexibility

```javascript
// Subscribe to all order events
const ws = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.order.*');

// Subscribe to all events
const ws2 = new WebSocket('ws://localhost:15500/pubsub/ws?topics=events.#');
```

### Handle Reconnection

```javascript
function createSubscription(topics, onMessage) {
  let ws;
  let reconnectDelay = 1000;
  
  function connect() {
    ws = new WebSocket(`ws://localhost:15500/pubsub/ws?topics=${topics.join(',')}`);
    
    ws.onopen = () => {
      reconnectDelay = 1000;
      console.log('Subscribed to topics');
    };
    
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      onMessage(msg);
    };
    
    ws.onclose = () => {
      console.log('Disconnected, reconnecting...');
      setTimeout(connect, reconnectDelay);
      reconnectDelay = Math.min(reconnectDelay * 2, 30000);
    };
  }
  
  connect();
  return () => ws.close();
}
```

### Keep Messages Small

Pub/Sub is fire-and-forget. Keep messages under 1MB:

```python
# Good: Small message with reference
event_data = json.dumps({
    "order_id": order_id,
    "user_id": user_id,
    "data_ref": f"/orders/{order_id}"  # Reference to full data
})

# Bad: Large message
event_data = json.dumps({
    "order_id": order_id,
    "user_id": user_id,
    "items": [...],  # Large array
    "full_data": {...}  # Large object
})
```

## Related Topics

- [Publishing to Topics](../pubsub/PUBLISHING.md) - Publishing messages
- [Subscribing to Topics](../pubsub/SUBSCRIBING.md) - WebSocket subscriptions
- [Wildcards](../pubsub/WILDCARDS.md) - Pattern matching

