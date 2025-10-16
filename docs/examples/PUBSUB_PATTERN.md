# Pub/Sub Messaging Pattern Sample

## Overview

This example demonstrates topic-based publish/subscribe messaging for building event-driven architectures and notification systems.

## Use Case

An e-commerce platform with multiple services that need to react to various events:
- Order service publishes order events
- Email service sends notifications
- SMS service sends alerts
- Analytics service tracks metrics
- Inventory service updates stock

## Architecture

```
┌─────────────────────────────────────────┐
│         Event Publishers                │
│  ┌────────┐  ┌────────┐  ┌────────┐    │
│  │Orders  │  │Payments│  │ Users  │    │
│  └────────┘  └────────┘  └────────┘    │
└─────────────────────────────────────────┘
         │          │          │
         └──────────┼──────────┘
                    │ Publish to Topics
         ┌──────────────────────────┐
         │     Synap Pub/Sub        │
         │   Topic Router           │
         │                          │
         │  orders.#                │
         │  payments.#              │
         │  notifications.*         │
         └──────────────────────────┘
                    │ Route to Subscribers
         ┌──────────┼──────────┐
         │          │          │
┌────────────┐ ┌────────────┐ ┌────────────┐
│   Email    │ │    SMS     │ │ Analytics  │
│  Service   │ │  Service   │ │  Service   │
└────────────┘ └────────────┘ └────────────┘
```

## Topic Hierarchy

```
orders.
  ├─ created
  ├─ updated
  ├─ cancelled
  ├─ fulfilled
  └─ refunded

payments.
  ├─ initiated
  ├─ completed
  ├─ failed
  └─ refunded

notifications.
  ├─ email.
  │  ├─ user
  │  └─ admin
  ├─ sms.
  │  ├─ user
  │  └─ admin
  └─ push

users.
  ├─ registered
  ├─ updated
  ├─ deleted
  └─ login
```

## Publisher Implementation

### TypeScript Publisher

```typescript
import { SynapClient } from '@hivellm/synap-client';

class EventPublisher {
  private synap: SynapClient;
  
  constructor(synapUrl: string, apiKey: string) {
    this.synap = new SynapClient({ url: synapUrl, apiKey });
  }
  
  async publishOrderCreated(order: any) {
    await this.synap.pubsub.publish(
      'orders.created',
      {
        orderId: order.id,
        userId: order.userId,
        items: order.items,
        total: order.total,
        timestamp: Date.now()
      }
    );
  }
  
  async publishPaymentCompleted(payment: any) {
    await this.synap.pubsub.publish(
      'payments.completed',
      {
        paymentId: payment.id,
        orderId: payment.orderId,
        amount: payment.amount,
        method: payment.method,
        timestamp: Date.now()
      }
    );
  }
  
  async sendEmailNotification(to: string, subject: string, body: string) {
    await this.synap.pubsub.publish(
      'notifications.email.user',
      { to, subject, body, timestamp: Date.now() }
    );
  }
  
  async sendSMSNotification(to: string, message: string) {
    await this.synap.pubsub.publish(
      'notifications.sms.user',
      { to, message, timestamp: Date.now() }
    );
  }
}

// Usage in Order Service
const publisher = new EventPublisher(
  'http://localhost:15500',
  process.env.SYNAP_API_KEY
);

app.post('/api/orders', async (req, res) => {
  const order = await createOrder(req.body);
  
  // Publish order created event
  await publisher.publishOrderCreated(order);
  
  res.json({ success: true, orderId: order.id });
});
```

### Python Publisher

```python
from synap import AsyncSynapClient

class EventPublisher:
    def __init__(self, synap_url: str, api_key: str):
        self.client = AsyncSynapClient(url=synap_url, api_key=api_key)
    
    async def publish_order_created(self, order: dict):
        await self.client.pubsub.publish(
            'orders.created',
            {
                'order_id': order['id'],
                'user_id': order['user_id'],
                'items': order['items'],
                'total': order['total'],
                'timestamp': time.time()
            }
        )
    
    async def publish_payment_completed(self, payment: dict):
        await self.client.pubsub.publish(
            'payments.completed',
            {
                'payment_id': payment['id'],
                'order_id': payment['order_id'],
                'amount': payment['amount'],
                'timestamp': time.time()
            }
        )
```

## Subscriber Implementation

### Email Service (TypeScript)

```typescript
class EmailService {
  private synap: SynapClient;
  
  constructor(synapUrl: string, apiKey: string) {
    this.synap = new SynapClient({ url: synapUrl, apiKey });
  }
  
  async start() {
    console.log('Email service starting...');
    
    // Subscribe to email notifications and order events
    await this.synap.pubsub.subscribe(
      [
        'notifications.email.*',  // All email notifications
        'orders.created',         // New orders
        'payments.completed'      // Completed payments
      ],
      (topic, message) => this.handleMessage(topic, message)
    );
    
    console.log('Email service listening for events');
  }
  
  private async handleMessage(topic: string, message: any) {
    try {
      if (topic.startsWith('notifications.email')) {
        await this.sendEmail(message);
      } else if (topic === 'orders.created') {
        await this.sendOrderConfirmation(message);
      } else if (topic === 'payments.completed') {
        await this.sendPaymentReceipt(message);
      }
    } catch (error) {
      console.error(`Error handling ${topic}:`, error);
    }
  }
  
  private async sendEmail(data: any) {
    console.log(`Sending email to ${data.to}: ${data.subject}`);
    // Email sending logic
  }
  
  private async sendOrderConfirmation(order: any) {
    console.log(`Sending order confirmation for ${order.orderId}`);
    // Fetch user email and send confirmation
  }
  
  private async sendPaymentReceipt(payment: any) {
    console.log(`Sending payment receipt for ${payment.paymentId}`);
    // Send receipt email
  }
}

// Run service
const service = new EmailService(
  'http://localhost:15500',
  process.env.SYNAP_API_KEY
);

service.start();
```

### Analytics Service (Python)

```python
class AnalyticsService:
    def __init__(self, synap_url: str):
        self.client = AsyncSynapClient(url=synap_url)
    
    async def start(self):
        print('Analytics service starting...')
        
        # Subscribe to all events
        await self.client.pubsub.subscribe(
            ['#'],  # Wildcard: all topics
            self.handle_event
        )
        
        print('Analytics service listening...')
    
    def handle_event(self, topic: str, message: dict):
        asyncio.create_task(self._track_event(topic, message))
    
    async def _track_event(self, topic: str, message: dict):
        if topic.startswith('orders.'):
            await self.track_order_event(topic, message)
        elif topic.startswith('payments.'):
            await self.track_payment_event(topic, message)
        elif topic.startswith('users.'):
            await self.track_user_event(topic, message)
    
    async def track_order_event(self, topic: str, message: dict):
        event_type = topic.split('.')[-1]
        
        await self.send_to_analytics({
            'event': f'order_{event_type}',
            'order_id': message.get('order_id'),
            'total': message.get('total'),
            'timestamp': message.get('timestamp')
        })
    
    async def track_payment_event(self, topic: str, message: dict):
        # Track revenue metrics
        pass
    
    async def track_user_event(self, topic: str, message: dict):
        # Track user activity
        pass
    
    async def send_to_analytics(self, event: dict):
        print(f'Tracking: {event}')
        # Send to analytics platform (Mixpanel, Amplitude, etc.)
```

### Inventory Service (Rust)

```rust
pub struct InventoryService {
    client: SynapClient,
}

impl InventoryService {
    pub async fn new(synap_url: &str) -> Result<Self> {
        Ok(Self {
            client: SynapClient::connect(synap_url).await?,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        println!("Inventory service starting...");
        
        // Subscribe to order events
        self.client.pubsub_subscribe(
            &["orders.created", "orders.cancelled"],
            |topic, message| async move {
                match topic.as_str() {
                    "orders.created" => Self::reserve_inventory(message).await,
                    "orders.cancelled" => Self::release_inventory(message).await,
                    _ => Ok(())
                }
            }
        ).await?;
        
        println!("Inventory service listening...");
        
        tokio::signal::ctrl_c().await?;
        Ok(())
    }
    
    async fn reserve_inventory(message: serde_json::Value) -> Result<()> {
        let order_id = message["orderId"].as_str().unwrap();
        let items = message["items"].as_array().unwrap();
        
        println!("Reserving inventory for order {}", order_id);
        
        // Reserve items logic
        for item in items {
            let item_id = item["id"].as_str().unwrap();
            let quantity = item["quantity"].as_i64().unwrap();
            
            // Update inventory count
            // (This would be actual inventory logic)
        }
        
        Ok(())
    }
    
    async fn release_inventory(message: serde_json::Value) -> Result<()> {
        println!("Releasing inventory for cancelled order");
        Ok(())
    }
}
```

## Wildcard Subscriptions

### Single-Level Wildcard (*)

```typescript
// Subscribe to all email notifications
await synap.pubsub.subscribe(
  ['notifications.email.*'],
  (topic, message) => {
    // Receives:
    //   notifications.email.user
    //   notifications.email.admin
    // But NOT:
    //   notifications.email.user.welcome
  }
);
```

### Multi-Level Wildcard (#)

```typescript
// Subscribe to all order events
await synap.pubsub.subscribe(
  ['orders.#'],
  (topic, message) => {
    // Receives:
    //   orders.created
    //   orders.payment.completed
    //   orders.shipping.dispatched
    //   (any topic starting with "orders.")
  }
);
```

### Multiple Patterns

```typescript
await synap.pubsub.subscribe(
  [
    'orders.#',              // All order events
    'payments.#',            // All payment events
    'notifications.email.*'  // Direct email notifications only
  ],
  handleEvent
);
```

## Fan-Out Pattern

### One Publisher, Many Subscribers

```typescript
// Publisher (Order Service)
await synap.pubsub.publish('orders.created', orderData);

// Subscriber 1: Email Service
await synap.pubsub.subscribe(['orders.#'], sendOrderEmail);

// Subscriber 2: SMS Service
await synap.pubsub.subscribe(['orders.#'], sendOrderSMS);

// Subscriber 3: Analytics
await synap.pubsub.subscribe(['orders.#'], trackOrder);

// Subscriber 4: Inventory
await synap.pubsub.subscribe(['orders.created'], reserveInventory);

// All 4 subscribers receive the same message
```

## Request-Response Pattern

### Using Correlation IDs

```typescript
// Service A: Request
const correlationId = uuidv4();

await synap.pubsub.publish('service.b.requests', {
  correlationId,
  action: 'process_data',
  data: {...}
});

// Subscribe to responses
await synap.pubsub.subscribe(
  [`service.a.responses.${correlationId}`],
  (topic, response) => {
    console.log('Got response:', response);
  }
);

// Service B: Response
await synap.pubsub.subscribe(
  ['service.b.requests'],
  async (topic, request) => {
    const result = await processData(request.data);
    
    // Publish response
    await synap.pubsub.publish(
      `service.a.responses.${request.correlationId}`,
      { result, status: 'success' }
    );
  }
);
```

## Aggregator Pattern

```python
class OrderAggregator:
    def __init__(self, synap_url: str):
        self.client = AsyncSynapClient(url=synap_url)
        self.partial_orders = {}
    
    async def start(self):
        await self.client.pubsub.subscribe(
            ['orders.#'],
            self.aggregate_order_events
        )
    
    def aggregate_order_events(self, topic: str, message: dict):
        order_id = message.get('order_id')
        
        if order_id not in self.partial_orders:
            self.partial_orders[order_id] = {
                'created': False,
                'paid': False,
                'shipped': False
            }
        
        event_type = topic.split('.')[-1]
        
        if event_type == 'created':
            self.partial_orders[order_id]['created'] = True
        elif event_type == 'payment_completed':
            self.partial_orders[order_id]['paid'] = True
        elif event_type == 'shipped':
            self.partial_orders[order_id]['shipped'] = True
        
        # Check if order is complete
        order_state = self.partial_orders[order_id]
        if all(order_state.values()):
            print(f'Order {order_id} fully completed!')
            asyncio.create_task(self.finalize_order(order_id))
            del self.partial_orders[order_id]
    
    async def finalize_order(self, order_id: str):
        # Send completion notification
        await self.client.pubsub.publish(
            'orders.fully_completed',
            {'order_id': order_id, 'timestamp': time.time()}
        )
```

## Filtering Pattern

### Server-Side Filtering

Use topic hierarchy:

```typescript
// Only critical alerts
await synap.pubsub.subscribe(
  ['alerts.critical.*'],
  handleCriticalAlert
);

// All alerts
await synap.pubsub.subscribe(
  ['alerts.#'],
  handleAllAlerts
);
```

### Client-Side Filtering

```typescript
await synap.pubsub.subscribe(
  ['events.#'],
  (topic, message) => {
    // Filter by message content
    if (message.userId === currentUserId) {
      handleUserEvent(message);
    }
  }
);
```

## Testing

### Mock Publisher

```typescript
describe('Pub/Sub', () => {
  it('should deliver message to subscriber', async () => {
    const synap = new SynapClient({ url: 'http://localhost:15500' });
    
    let received = false;
    
    // Subscribe
    await synap.pubsub.subscribe(
      ['test.topic'],
      (topic, message) => {
        received = true;
        expect(message).toEqual({ data: 'test' });
      }
    );
    
    // Publish
    await synap.pubsub.publish('test.topic', { data: 'test' });
    
    // Wait for delivery
    await new Promise(resolve => setTimeout(resolve, 100));
    
    expect(received).toBe(true);
  });
});
```

## Best Practices

### Topic Naming

```
<domain>.<entity>.<action>.<specificity>

Good examples:
  orders.payment.completed
  users.account.deleted
  notifications.email.user.welcome

Bad examples:
  order_payment_completed  (use dots)
  USER_ACCOUNT_DELETED    (use lowercase)
  notif                    (be explicit)
```

### Message Structure

Include standard fields:

```typescript
interface StandardEvent {
  // Required
  timestamp: number;
  eventType: string;
  
  // Recommended
  eventId?: string;
  correlationId?: string;
  userId?: string;
  
  // Domain-specific
  [key: string]: any;
}
```

### Error Handling

Make subscribers resilient:

```python
async def resilient_handler(topic: str, message: dict):
    try:
        await process_message(message)
    except Exception as e:
        # Log error but don't crash
        logger.error(f'Error processing {topic}: {e}')
        
        # Optionally: send to dead letter or retry
```

## Performance Considerations

### High-Frequency Events

For metrics/telemetry, use sampling:

```typescript
let eventCount = 0;

function publishMetric(metric: any) {
  eventCount++;
  
  // Sample 1% of events
  if (eventCount % 100 === 0) {
    synap.pubsub.publish('metrics.sampled', metric);
  }
}
```

### Large Subscriber Count

Synap efficiently handles fan-out:
- 1K+ subscribers per topic supported
- Parallel delivery to all subscribers
- Sub-millisecond routing

## Monitoring

### Subscription Health

```typescript
// Monitor subscription
const subscription = await synap.pubsub.subscribe(
  ['orders.#'],
  handler
);

setInterval(() => {
  console.log('Subscription active:', subscription.isActive);
}, 10000);
```

### Message Counts

```typescript
let messageCount = 0;

await synap.pubsub.subscribe(
  ['#'],  // All topics
  (topic, message) => {
    messageCount++;
    console.log(`Processed ${messageCount} messages`);
  }
);
```

## See Also

- [PUBSUB.md](../specs/PUBSUB.md) - Pub/Sub specification
- [EVENT_BROADCAST.md](EVENT_BROADCAST.md) - Event broadcasting
- [CHAT_SAMPLE.md](CHAT_SAMPLE.md) - Chat example

