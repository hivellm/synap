# @hivellm/synap

Official TypeScript/JavaScript SDK for [Synap](https://github.com/hivellm/synap) - High-Performance In-Memory Key-Value Store & Message Broker.

## Features

✅ **Full TypeScript support** with complete type definitions  
✅ **StreamableHTTP protocol** implementation  
✅ **Key-Value operations** (GET, SET, DELETE, INCR, SCAN, etc.)  
✅ **Queue system** (publish, consume, ACK/NACK)  
✅ **Event Streams** - Append-only event logs with replay ✨ NEW  
✅ **Pub/Sub** - Topic-based message routing ✨ NEW  
✅ **Reactive Patterns** with RxJS - Observable-based consumption  
✅ **Authentication** (Basic Auth + API Keys)  
✅ **Gzip compression** support  
✅ **Modern ESM/CJS** dual package  
✅ **Minimal dependencies** (uuid + RxJS)  
✅ **Node.js 18+** and browser compatible  

---

## Installation

```bash
npm install @hivellm/synap
# or
yarn add @hivellm/synap
# or
pnpm add @hivellm/synap
```

---

## Quick Start

### Basic Usage

```typescript
import { Synap } from '@hivellm/synap';

// Create client
const synap = new Synap({
  url: 'http://localhost:15500'
});

// Key-Value operations
await synap.kv.set('user:1', { name: 'Alice', age: 30 });
const user = await synap.kv.get('user:1');
console.log(user); // { name: 'Alice', age: 30 }

// Queue operations (traditional)
await synap.queue.createQueue('jobs');
const msgId = await synap.queue.publishString('jobs', 'process-video');
const { message, text } = await synap.queue.consumeString('jobs', 'worker-1');
await synap.queue.ack('jobs', message.id);

// Reactive queue consumption (recommended)
synap.queue.processMessages({
  queueName: 'jobs',
  consumerId: 'worker-1',
  concurrency: 5
}, async (data) => {
  console.log('Processing:', data);
  // Auto-ACK on success, auto-NACK on error
}).subscribe();

// Event Stream operations
await synap.stream.createRoom('chat-room');
await synap.stream.publish('chat-room', 'message.sent', { text: 'Hello!' });

// Reactive stream consumption
synap.stream.observeEvents({
  roomName: 'chat-room',
  subscriberId: 'user-1',
  fromOffset: 0
}).subscribe({
  next: (event) => console.log(event.event, event.data)
});

// Pub/Sub operations
await synap.pubsub.publish('user.created', { id: 123, name: 'Alice' });
```

---

## Key-Value Store

### SET/GET Operations

```typescript
// Set a value
await synap.kv.set('mykey', 'myvalue');

// Set with TTL (expires in 60 seconds)
await synap.kv.set('session:abc', { userId: 123 }, { ttl: 60 });

// Get a value
const value = await synap.kv.get('mykey'); // 'myvalue'

// Get with type safety
const session = await synap.kv.get<{ userId: number }>('session:abc');

// Delete a key
const deleted = await synap.kv.del('mykey'); // true

// Check if key exists
const exists = await synap.kv.exists('mykey'); // false
```

### Atomic Operations

```typescript
// Increment
await synap.kv.set('counter', 0);
await synap.kv.incr('counter', 5); // 5
await synap.kv.incr('counter', 3); // 8

// Decrement
await synap.kv.decr('counter', 2); // 6
```

### Batch Operations

```typescript
// Set multiple keys
await synap.kv.mset({
  'user:1': { name: 'Alice' },
  'user:2': { name: 'Bob' },
  'user:3': { name: 'Charlie' }
});

// Get multiple keys
const users = await synap.kv.mget(['user:1', 'user:2', 'user:3']);
// { 'user:1': {...}, 'user:2': {...}, 'user:3': {...} }

// Delete multiple keys
const deleted = await synap.kv.mdel(['user:1', 'user:2', 'user:3']); // 3
```

### Scanning & Discovery

```typescript
// Scan with prefix
const result = await synap.kv.scan('user:', 100);
console.log(result.keys); // ['user:1', 'user:2', ...]

// List all keys matching pattern
const keys = await synap.kv.keys('user:*');

// Get database size
const size = await synap.kv.dbsize(); // number of keys
```

### TTL Management

```typescript
// Set expiration (60 seconds)
await synap.kv.expire('mykey', 60);

// Get TTL
const ttl = await synap.kv.ttl('mykey'); // seconds remaining or null

// Remove expiration
await synap.kv.persist('mykey');
```

---

## Queue System

### Creating Queues

```typescript
// Default configuration
await synap.queue.createQueue('tasks');

// Custom configuration
await synap.queue.createQueue('jobs', {
  max_depth: 10000,
  ack_deadline_secs: 30,
  default_max_retries: 3,
  default_priority: 5
});
```

### Publishing Messages

```typescript
// Publish string message
const msgId = await synap.queue.publishString('tasks', 'process-video-123');

// Publish JSON message
const msgId = await synap.queue.publishJSON('tasks', {
  task: 'send-email',
  to: 'user@example.com',
  subject: 'Welcome!'
});

// Publish with priority (0-9, where 9 is highest)
await synap.queue.publishString('tasks', 'urgent-task', { priority: 9 });

// Publish with custom retries
await synap.queue.publishString('tasks', 'retry-task', { max_retries: 5 });

// Publish raw bytes
const bytes = new Uint8Array([1, 2, 3, 4, 5]);
await synap.queue.publish('tasks', bytes);
```

### Consuming Messages

```typescript
// Consume string message
const { message, text } = await synap.queue.consumeString('tasks', 'worker-1');

if (message) {
  console.log('Message:', text);
  console.log('Priority:', message.priority);
  console.log('Retry count:', message.retry_count);
  
  // Acknowledge successful processing
  await synap.queue.ack('tasks', message.id);
}

// Consume JSON message
const { message, data } = await synap.queue.consumeJSON<{ task: string }>('tasks', 'worker-1');

if (message) {
  console.log('Task:', data.task);
  await synap.queue.ack('tasks', message.id);
}

// Consume raw bytes
const message = await synap.queue.consume('tasks', 'worker-1');
if (message) {
  console.log('Payload:', message.payload); // Uint8Array
}
```

### ACK/NACK Operations

```typescript
const { message } = await synap.queue.consumeString('tasks', 'worker-1');

if (message) {
  try {
    // Process the message
    await processTask(message);
    
    // Acknowledge success (ACK)
    await synap.queue.ack('tasks', message.id);
  } catch (error) {
    // Requeue for retry (NACK)
    await synap.queue.nack('tasks', message.id, true);
    
    // Or send to DLQ without requeueing
    // await synap.queue.nack('tasks', message.id, false);
  }
}
```

### Queue Management

```typescript
// List all queues
const queues = await synap.queue.listQueues();
console.log('Queues:', queues);

// Get queue statistics
const stats = await synap.queue.stats('tasks');
console.log('Depth:', stats.depth);
console.log('Published:', stats.published);
console.log('Consumed:', stats.consumed);

// Purge all messages from queue
const purged = await synap.queue.purge('tasks');
console.log(`Purged ${purged} messages`);

// Delete queue
await synap.queue.deleteQueue('tasks');
```

---

## Reactive Queues (RxJS)

The SDK provides reactive queue consumption patterns using RxJS for better composability, error handling, and concurrency control.

### Why Reactive?

Traditional polling-based consumption:
- ❌ Requires manual polling loops
- ❌ Limited concurrency control
- ❌ Complex error handling
- ❌ Hard to compose with other operations

Reactive consumption:
- ✅ Event-driven, non-blocking
- ✅ Built-in concurrency support
- ✅ Rich operator library (retry, filter, map, etc.)
- ✅ Easy to compose and test
- ✅ Better observability

### Basic Reactive Consumer

```typescript
// Simple consumer with manual ACK/NACK
synap.queue.observeMessages({
  queueName: 'tasks',
  consumerId: 'worker-1',
  pollingInterval: 500,
  concurrency: 5
}).subscribe({
  next: async (msg) => {
    console.log('Processing:', msg.data);
    
    try {
      await processTask(msg.data);
      await msg.ack();  // Acknowledge success
    } catch (error) {
      await msg.nack(); // Negative acknowledge (will retry)
    }
  },
  error: (err) => console.error('Error:', err)
});
```

### Auto-Processing with Handler

```typescript
// Automatic ACK/NACK handling
synap.queue.processMessages({
  queueName: 'emails',
  consumerId: 'email-worker',
  concurrency: 10
}, async (data, message) => {
  // Process message - auto-ACK on success, auto-NACK on error
  await sendEmail(data);
}).subscribe({
  next: (result) => {
    if (result.success) {
      console.log('✅ Processed:', result.messageId);
    } else {
      console.error('❌ Failed:', result.messageId);
    }
  }
});
```

### Advanced Reactive Patterns

**Priority-based processing:**
```typescript
import { filter } from 'rxjs/operators';

synap.queue.observeMessages({
  queueName: 'tasks',
  consumerId: 'priority-worker'
}).pipe(
  filter(msg => msg.message.priority >= 7) // Only high-priority
).subscribe(async (msg) => {
  await processFast(msg.data);
  await msg.ack();
});
```

**Batch processing:**
```typescript
import { bufferTime } from 'rxjs/operators';

synap.queue.observeMessages({
  queueName: 'analytics',
  consumerId: 'batch-worker',
  pollingInterval: 100
}).pipe(
  bufferTime(5000) // Collect messages for 5 seconds
).subscribe(async (batch) => {
  await processBatch(batch.map(m => m.data));
  await Promise.all(batch.map(m => m.ack()));
});
```

**Type-based routing:**
```typescript
const messages$ = synap.queue.observeMessages({ queueName: 'mixed', consumerId: 'router' });

// Email handler
messages$.pipe(filter(m => m.data.type === 'email'))
  .subscribe(async (msg) => { await sendEmail(msg.data); await msg.ack(); });

// Notification handler  
messages$.pipe(filter(m => m.data.type === 'notification'))
  .subscribe(async (msg) => { await sendNotification(msg.data); await msg.ack(); });
```

**Queue monitoring:**
```typescript
// Monitor queue stats every 3 seconds
synap.queue.observeStats('tasks', 3000).subscribe({
  next: (stats) => {
    console.log(`Depth: ${stats.depth}, Acked: ${stats.acked}`);
  }
});
```

### Graceful Shutdown

```typescript
const subscription = synap.queue.processMessages({
  queueName: 'tasks',
  consumerId: 'worker-1',
  concurrency: 5
}, processTask).subscribe();

process.on('SIGINT', () => {
  console.log('Shutting down...');
  
  // Stop consuming new messages
  synap.queue.stopConsumer('tasks', 'worker-1');
  
  // Wait for current messages to finish
  setTimeout(() => {
    subscription.unsubscribe();
    synap.close();
    process.exit(0);
  }, 2000);
});
```

### Consumer Options

```typescript
interface QueueConsumerOptions {
  queueName: string;       // Queue to consume from
  consumerId: string;      // Unique consumer ID
  pollingInterval?: number; // Poll interval in ms (default: 1000)
  concurrency?: number;     // Max concurrent messages (default: 1)
  autoAck?: boolean;        // Auto-acknowledge (default: false)
  autoNack?: boolean;       // Auto-nack on error (default: false)
  requeueOnNack?: boolean;  // Requeue on nack (default: true)
}
```

📖 **See [REACTIVE_QUEUES.md](./REACTIVE_QUEUES.md) for complete reactive patterns guide**

---

## Event Streams

Event Streams provide append-only event logs with the ability to replay events from any offset.

### Basic Operations

```typescript
// Create a stream room
await synap.stream.createRoom('chat-room');

// Publish events
const offset1 = await synap.stream.publish('chat-room', 'message.sent', {
  user: 'Alice',
  text: 'Hello!',
  timestamp: Date.now()
});

// Consume events from offset
const events = await synap.stream.consume('chat-room', 'subscriber-1', 0);
events.forEach(event => {
  console.log(`[${event.offset}] ${event.event}:`, event.data);
});

// Get stream statistics
const stats = await synap.stream.stats('chat-room');
console.log(`Events: ${stats.event_count}, Subscribers: ${stats.subscribers}`);
```

### Reactive Stream Consumption

```typescript
// Subscribe to all events
synap.stream.observeEvents({
  roomName: 'chat-room',
  subscriberId: 'user-1',
  fromOffset: 0,
  pollingInterval: 500
}).subscribe({
  next: (event) => {
    console.log(`[${event.offset}] ${event.event}:`, event.data);
  }
});

// Filter specific event types
synap.stream.observeEvent({
  roomName: 'notifications',
  subscriberId: 'user-1',
  eventName: 'notification.important'
}).subscribe({
  next: (event) => console.log('Important:', event.data)
});

// Monitor stream stats in real-time
synap.stream.observeStats('chat-room', 3000).subscribe({
  next: (stats) => console.log('Event count:', stats.event_count)
});
```

### Event Replay

```typescript
// Replay events from beginning
synap.stream.observeEvents({
  roomName: 'audit-log',
  subscriberId: 'auditor',
  fromOffset: 0  // Start from beginning
}).subscribe({
  next: (event) => console.log('Replaying:', event)
});

// Resume from last known offset
const lastOffset = 42;
synap.stream.observeEvents({
  roomName: 'chat-room',
  subscriberId: 'user-1',
  fromOffset: lastOffset + 1
}).subscribe({
  next: (event) => console.log('New event:', event)
});
```

### Stream Patterns

```typescript
import { filter, bufferTime, map } from 'rxjs/operators';

// Event aggregation
synap.stream.observeEvents({ roomName: 'analytics' }).pipe(
  bufferTime(5000),
  map(events => ({ count: events.length, events }))
).subscribe({
  next: (batch) => console.log(`Batch: ${batch.count} events`)
});

// Filter by event properties
synap.stream.observeEvents<{ priority: number }>({ roomName: 'tasks' }).pipe(
  filter(event => event.data.priority > 7)
).subscribe({
  next: (event) => console.log('High priority:', event)
});
```

---

## Pub/Sub

Pub/Sub provides topic-based message routing with support for wildcard subscriptions.

### Publishing

```typescript
// Publish to a topic
await synap.pubsub.publish('user.created', {
  userId: '123',
  name: 'Alice',
  email: 'alice@example.com'
});

// Publish with priority
await synap.pubsub.publish('alerts.critical', {
  message: 'System down!'
}, { priority: 9 });

// Publish with headers
await synap.pubsub.publish('events.custom', {
  data: 'value'
}, {
  headers: {
    'content-type': 'application/json',
    'source': 'api-gateway'
  }
});
```

### Topic Patterns

```typescript
// Simple topics
'user.created'
'order.completed'
'payment.failed'

// Hierarchical topics
'app.users.created'
'app.orders.completed'
'app.payments.failed'

// Wildcard patterns (subscription)
'user.*'           // Matches: user.created, user.updated, user.deleted
'*.error'          // Matches: app.error, db.error, api.error
'app.*.event'      // Matches: app.user.event, app.order.event
```

### Reactive Subscription

```typescript
// Subscribe to multiple topics
synap.pubsub.subscribe({
  topics: ['user.created', 'user.updated', 'user.deleted'],
  subscriberId: 'user-service'
}).subscribe({
  next: (message) => {
    console.log(`Topic: ${message.topic}`);
    console.log(`Data:`, message.data);
  }
});

// Subscribe to single topic
synap.pubsub.subscribeTopic('orders.created').subscribe({
  next: (message) => {
    console.log('New order:', message.data);
  }
});

// Subscribe with wildcard
synap.pubsub.subscribe({
  topics: ['user.*', '*.error'],
  subscriberId: 'monitor'
}).subscribe({
  next: (message) => {
    if (message.topic.endsWith('.error')) {
      console.error('Error event:', message.data);
    }
  }
});
```

### Unsubscribing

```typescript
const subscriberId = 'my-subscriber';
const topics = ['user.*', 'order.*'];

// Unsubscribe from specific topics
synap.pubsub.unsubscribe(subscriberId, topics);

// Unsubscribe from all topics
synap.pubsub.unsubscribeAll();
```

📖 **See [examples/stream-patterns.ts](./examples/stream-patterns.ts) and [examples/pubsub-patterns.ts](./examples/pubsub-patterns.ts) for more patterns**

---

## Authentication

### Basic Auth (Username/Password)

```typescript
const synap = new Synap({
  url: 'http://localhost:15500',
  auth: {
    type: 'basic',
    username: 'admin',
    password: 'your-password'
  }
});

await synap.kv.set('protected:key', 'secure-value');
```

### API Key Auth

```typescript
const synap = new Synap({
  url: 'http://localhost:15500',
  auth: {
    type: 'api_key',
    apiKey: 'sk_YOUR_API_KEY_HERE'
  }
});

await synap.queue.publishString('secure-queue', 'message');
```

---

## Error Handling

```typescript
import { SynapError, NetworkError, ServerError, TimeoutError } from '@hivellm/synap';

try {
  await synap.kv.set('mykey', 'value');
} catch (error) {
  if (error instanceof NetworkError) {
    console.error('Network error:', error.message);
  } else if (error instanceof ServerError) {
    console.error('Server error:', error.message, error.statusCode);
  } else if (error instanceof TimeoutError) {
    console.error('Request timeout:', error.timeoutMs);
  } else if (error instanceof SynapError) {
    console.error('Synap error:', error.message);
  }
}
```

---

## Advanced Usage

### Custom Timeouts

```typescript
const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 10000 // 10 seconds
});
```

### Debug Mode

```typescript
const synap = new Synap({
  url: 'http://localhost:15500',
  debug: true // Logs all requests/responses
});
```

### Direct Client Access

```typescript
const synap = new Synap();

// Access underlying HTTP client
const client = synap.getClient();

// Send custom command
const response = await client.sendCommand('custom.command', {
  param1: 'value1',
  param2: 'value2'
});
```

---

## Examples

### Simple Cache

```typescript
async function cacheExample() {
  const synap = new Synap();
  
  // Set cache with TTL
  await synap.kv.set('cache:user:123', {
    name: 'Alice',
    email: 'alice@example.com'
  }, { ttl: 3600 }); // 1 hour
  
  // Get from cache
  const cached = await synap.kv.get('cache:user:123');
  if (cached) {
    console.log('Cache hit!', cached);
  } else {
    console.log('Cache miss');
  }
}
```

### Task Queue Worker

```typescript
async function worker() {
  const synap = new Synap();
  const QUEUE_NAME = 'tasks';
  const WORKER_ID = 'worker-1';
  
  while (true) {
    const { message, data } = await synap.queue.consumeJSON(QUEUE_NAME, WORKER_ID);
    
    if (!message) {
      await new Promise(resolve => setTimeout(resolve, 1000));
      continue;
    }
    
    try {
      // Process task
      await processTask(data);
      
      // ACK on success
      await synap.queue.ack(QUEUE_NAME, message.id);
    } catch (error) {
      // NACK on failure (will retry)
      await synap.queue.nack(QUEUE_NAME, message.id, true);
    }
  }
}
```

### Priority Queue

```typescript
async function priorityExample() {
  const synap = new Synap();
  
  await synap.queue.createQueue('priority-queue');
  
  // Publish with different priorities
  await synap.queue.publishString('priority-queue', 'Low priority', { priority: 1 });
  await synap.queue.publishString('priority-queue', 'Medium priority', { priority: 5 });
  await synap.queue.publishString('priority-queue', 'High priority', { priority: 9 });
  
  // Messages will be consumed in priority order (9, 5, 1)
  const { text: first } = await synap.queue.consumeString('priority-queue', 'worker');
  console.log(first); // 'High priority'
}
```

---

## API Reference

See [API Documentation](./docs/API.md) for complete API reference.

---

## TypeScript Support

The SDK is written in TypeScript and provides full type safety:

```typescript
import { Synap, QueueMessage, KVStats } from '@hivellm/synap';

const synap = new Synap();

// Type-safe KV operations
const user = await synap.kv.get<{ name: string; age: number }>('user:1');
if (user) {
  console.log(user.name); // TypeScript knows this is a string
}

// Type-safe Queue operations
const { message, data } = await synap.queue.consumeJSON<{
  task: string;
  priority: number;
}>('jobs', 'worker');

if (data) {
  console.log(data.task); // TypeScript infers the type
}
```

---

## Browser Support

The SDK works in modern browsers (ES2022+):

```html
<script type="module">
  import { Synap } from 'https://cdn.jsdelivr.net/npm/@hivellm/synap/+esm';
  
  const synap = new Synap({ url: 'http://localhost:15500' });
  await synap.kv.set('browser:key', 'value');
  const value = await synap.kv.get('browser:key');
  console.log(value);
</script>
```

---

## Development

```bash
# Install dependencies
npm install

# Build
npm run build

# Run tests (unit tests - no server required)
npm test                   # Default: unit tests with mocks
npm run test:unit          # Unit tests (fast, no server)
npm run test:s2s           # S2S tests (requires server)
npm run test:all           # All tests (unit + s2s)

# Watch mode
npm run dev
npm run test:watch

# Coverage
npm run test:coverage

# Lint
npm run lint

# Format
npm run format
```

### Testing Strategy

The SDK uses a **dual testing approach**:

**1. Unit Tests (Mock)** - No server required ✅
- 47 tests using mocked client
- Fast execution (~1 second)
- Perfect for CI/CD and development
- Run with: `npm test` or `npm run test:unit`

**2. S2S Tests (Server-to-Server)** - Optional ⚙️
- 68 integration tests with real server
- Requires Synap server running on `localhost:15500`
- Run with: `npm run test:s2s`

**Total: 115 tests - 100% passing**

See [TESTING.md](./src/__tests__/TESTING.md) for complete testing guide.

---

## License

MIT License - See [LICENSE](../../LICENSE) for details.

---

## Links

- [Synap Server](https://github.com/hivellm/synap)
- [Documentation](https://github.com/hivellm/synap/tree/main/docs)
- [Examples](./examples)
- [Changelog](./CHANGELOG.md)

---

## Support

- 🐛 [Report Bug](https://github.com/hivellm/synap/issues)
- 💡 [Request Feature](https://github.com/hivellm/synap/issues)
- 📖 [Documentation](https://github.com/hivellm/synap/tree/main/docs)

---

**Status**: ✅ Production Ready  
**Version**: 0.2.0-beta.1  
**Protocol**: StreamableHTTP  
**Node.js**: 18+

