# Event Broadcasting Sample

## Overview

This example demonstrates system-wide event broadcasting using Synap's Event Stream for real-time notifications across distributed services.

## Use Case

A microservices architecture where services need to be notified of system events:
- User registration
- Order processing
- Payment completion
- System alerts
- Audit logs

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    Event Publishers                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │   Auth   │  │  Orders  │  │ Payment  │              │
│  │ Service  │  │ Service  │  │ Service  │              │
│  └──────────┘  └──────────┘  └──────────┘              │
└──────────────────────────────────────────────────────────┘
         │              │              │
         └──────────────┼──────────────┘
                        │ Publish Events
         ┌──────────────────────────────────┐
         │         Synap Server             │
         │  Room: system-events             │
         └──────────────────────────────────┘
                        │ Broadcast
         ┌──────────────┼──────────────┐
         │              │              │
┌──────────────┐  ┌──────────┐  ┌──────────┐
│   Email      │  │   SMS    │  │Analytics │
│  Service     │  │ Service  │  │ Service  │
└──────────────┘  └──────────┘  └──────────┘
```

## Implementation

### Event Publisher (TypeScript)

```typescript
import { SynapClient } from '@hivellm/synap-client';

class EventBroadcaster {
  private synap: SynapClient;
  private serviceName: string;
  
  constructor(serviceName: string, synapUrl: string) {
    this.serviceName = serviceName;
    this.synap = new SynapClient({
      url: synapUrl,
      apiKey: process.env.SYNAP_API_KEY
    });
  }
  
  async publishEvent(
    eventType: string,
    data: any,
    metadata?: Record<string, string>
  ) {
    const result = await this.synap.stream.publish(
      'system-events',
      eventType,
      {
        ...data,
        service: this.serviceName,
        timestamp: Date.now()
      },
      metadata
    );
    
    console.log(`Event ${eventType} published at offset ${result.offset}`);
    return result;
  }
  
  async userRegistered(userId: string, email: string) {
    return this.publishEvent('user.registered', {
      userId,
      email,
      source: 'registration-flow'
    });
  }
  
  async orderCreated(orderId: string, userId: string, amount: number) {
    return this.publishEvent('order.created', {
      orderId,
      userId,
      amount,
      currency: 'USD'
    });
  }
  
  async paymentCompleted(paymentId: string, orderId: string, amount: number) {
    return this.publishEvent('payment.completed', {
      paymentId,
      orderId,
      amount,
      method: 'credit_card'
    });
  }
}

// Usage in Auth Service
const broadcaster = new EventBroadcaster('auth-service', 'http://synap:15500');

app.post('/api/register', async (req, res) => {
  const { email, password } = req.body;
  
  // Create user
  const user = await createUser(email, password);
  
  // Broadcast event
  await broadcaster.userRegistered(user.id, user.email);
  
  res.json({ success: true, userId: user.id });
});
```

### Event Consumer (Python)

```python
from synap import AsyncSynapClient
import asyncio

class EventConsumer:
    def __init__(self, service_name: str, synap_url: str):
        self.service_name = service_name
        self.client = AsyncSynapClient(url=synap_url)
        self.handlers = {}
    
    def on(self, event_type: str, handler):
        """Register event handler"""
        self.handlers[event_type] = handler
        return self
    
    async def start(self):
        """Start consuming events"""
        print(f'{self.service_name} listening for events...')
        
        def event_handler(event):
            handler = self.handlers.get(event.event_type)
            if handler:
                asyncio.create_task(handler(event.data))
        
        # Subscribe to system events
        subscription = await self.client.stream.subscribe(
            'system-events',
            event_handler,
            from_offset=None,  # Only new events
            replay=False
        )
        
        # Keep running
        try:
            await asyncio.Event().wait()
        finally:
            await subscription.unsubscribe()

# Email Service
class EmailService:
    def __init__(self, synap_url: str):
        self.consumer = EventConsumer('email-service', synap_url)
        
        # Register handlers
        self.consumer.on('user.registered', self.send_welcome_email)
        self.consumer.on('order.created', self.send_order_confirmation)
        self.consumer.on('payment.completed', self.send_payment_receipt)
    
    async def send_welcome_email(self, data: dict):
        print(f'Sending welcome email to {data["email"]}')
        # Send email logic
    
    async def send_order_confirmation(self, data: dict):
        print(f'Sending order confirmation for order {data["orderId"]}')
    
    async def send_payment_receipt(self, data: dict):
        print(f'Sending payment receipt for {data["paymentId"]}')
    
    async def run(self):
        await self.consumer.start()

# Run service
if __name__ == '__main__':
    service = EmailService('http://localhost:15500')
    asyncio.run(service.run())
```

### Analytics Consumer (Rust)

```rust
use synap_client::{SynapClient, StreamEvent};
use serde_json::Value;

pub struct AnalyticsService {
    client: SynapClient,
}

impl AnalyticsService {
    pub async fn new(synap_url: &str) -> Result<Self> {
        Ok(Self {
            client: SynapClient::connect(synap_url).await?,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Analytics service listening for events...");
        
        self.client.stream_subscribe(
            "system-events",
            |event: StreamEvent| async move {
                match event.event_type.as_str() {
                    "user.registered" => Self::track_registration(event.data).await,
                    "order.created" => Self::track_order(event.data).await,
                    "payment.completed" => Self::track_payment(event.data).await,
                    _ => {}
                }
            },
            None
        ).await?;
        
        // Keep running
        tokio::signal::ctrl_c().await?;
        Ok(())
    }
    
    async fn track_registration(data: Value) {
        tracing::info!("Tracking registration: {:?}", data);
        // Send to analytics platform
    }
    
    async fn track_order(data: Value) {
        tracing::info!("Tracking order: {:?}", data);
        // Record order metrics
    }
    
    async fn track_payment(data: Value) {
        tracing::info!("Tracking payment: {:?}", data);
        // Record revenue
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let service = AnalyticsService::new("http://localhost:15500").await?;
    service.start().await?;
    Ok(())
}
```

## Event Schema

### Standard Event Format

```typescript
interface SystemEvent {
  service: string;      // Originating service
  eventType: string;    // Event type
  timestamp: number;    // Unix timestamp (ms)
  data: any;           // Event-specific data
  metadata?: {
    correlationId?: string;
    userId?: string;
    sessionId?: string;
  };
}
```

### Event Types

```typescript
// User events
'user.registered'
'user.updated'
'user.deleted'
'user.login'
'user.logout'

// Order events
'order.created'
'order.updated'
'order.cancelled'
'order.fulfilled'

// Payment events
'payment.initiated'
'payment.completed'
'payment.failed'
'payment.refunded'

// System events
'system.startup'
'system.shutdown'
'system.error'
'system.alert'
```

## Multi-Service Orchestration

### Saga Pattern

```typescript
// Order Service
async function createOrder(userId: string, items: any[]) {
  const orderId = generateId();
  
  // Create order
  await saveOrder(orderId, userId, items);
  
  // Publish event
  await broadcaster.publishEvent('order.created', {
    orderId,
    userId,
    items,
    total: calculateTotal(items)
  });
  
  return orderId;
}

// Inventory Service (listens for order.created)
async function reserveInventory(event: any) {
  const { orderId, items } = event.data;
  
  try {
    await reserveItems(items);
    
    // Publish success
    await broadcaster.publishEvent('inventory.reserved', {
      orderId,
      items
    });
  } catch (error) {
    // Publish failure
    await broadcaster.publishEvent('inventory.reservation_failed', {
      orderId,
      reason: error.message
    });
  }
}

// Payment Service (listens for inventory.reserved)
async function processPayment(event: any) {
  const { orderId } = event.data;
  
  // Process payment
  const paymentId = await chargeCustomer(orderId);
  
  await broadcaster.publishEvent('payment.completed', {
    orderId,
    paymentId
  });
}
```

## Monitoring & Debugging

### Event Logging

```typescript
// Log all events to console
await synap.stream.subscribe(
  'system-events',
  (event) => {
    console.log(`[${event.eventType}] ${JSON.stringify(event.data)}`);
  }
);
```

### Event Replay for Debugging

```python
# Get last 1000 events for investigation
history = await client.stream.history(
    'system-events',
    from_offset=-1000,
    limit=1000
)

for event in history.events:
    if event.event_type == 'payment.failed':
        print(f'Failed payment: {event.data}')
```

## Error Handling

### Resilient Consumers

```rust
impl EventConsumer {
    async fn consume_with_retry(&self) -> Result<()> {
        loop {
            match self.subscribe_and_consume().await {
                Ok(_) => {
                    tracing::info!("Subscription ended normally");
                    break;
                }
                Err(e) => {
                    etracing::info!("Error consuming events: {}", e);
                    tracing::info!("Retrying in 5 seconds...");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
        Ok(())
    }
    
    async fn subscribe_and_consume(&self) -> Result<()> {
        let subscription = self.client.stream_subscribe(
            "system-events",
            |event| async move {
                self.handle_event(event).await.unwrap_or_else(|e| {
                    etracing::info!("Error handling event: {}", e);
                });
            },
            None
        ).await?;
        
        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;
        subscription.unsubscribe().await?;
        
        Ok(())
    }
}
```

## Best Practices

### Event Naming

Use hierarchical naming:
```
domain.entity.action

Examples:
  user.account.created
  order.payment.completed
  system.error.critical
```

### Event Versioning

Include version in event data:

```typescript
await synap.stream.publish('system-events', 'user.registered', {
  version: '1.0',
  userId: '123',
  email: 'user@example.com',
  // V1 specific fields
});
```

### Idempotency

Make event handlers idempotent:

```python
async def handle_user_registered(data: dict):
    user_id = data['userId']
    
    # Check if already processed (using KV store)
    processed = await client.kv.get(f'processed:user_registered:{user_id}')
    if processed.found:
        print('Event already processed, skipping')
        return
    
    # Process event
    await send_welcome_email(data['email'])
    
    # Mark as processed
    await client.kv.set(
        f'processed:user_registered:{user_id}',
        True,
        ttl=86400  # 24 hours
    )
```

## See Also

- [EVENT_STREAM.md](../specs/EVENT_STREAM.md) - Event stream specification
- [CHAT_SAMPLE.md](CHAT_SAMPLE.md) - Chat application example
- [PUBSUB_PATTERN.md](PUBSUB_PATTERN.md) - Pub/Sub patterns

