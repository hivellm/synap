# @synap/client

Official TypeScript/JavaScript SDK for [Synap](https://github.com/hivellm/synap) - High-Performance In-Memory Key-Value Store & Message Broker.

## Features

✅ **Full TypeScript support** with complete type definitions  
✅ **StreamableHTTP protocol** implementation  
✅ **Key-Value operations** (GET, SET, DELETE, INCR, SCAN, etc.)  
✅ **Queue system** (publish, consume, ACK/NACK)  
✅ **Authentication** (Basic Auth + API Keys)  
✅ **Gzip compression** support  
✅ **Modern ESM/CJS** dual package  
✅ **Zero dependencies** (except uuid)  
✅ **Node.js 18+** and browser compatible  

---

## Installation

```bash
npm install @synap/client
# or
yarn add @synap/client
# or
pnpm add @synap/client
```

---

## Quick Start

### Basic Usage

```typescript
import { Synap } from '@synap/client';

// Create client
const synap = new Synap({
  url: 'http://localhost:15500'
});

// Key-Value operations
await synap.kv.set('user:1', { name: 'Alice', age: 30 });
const user = await synap.kv.get('user:1');
console.log(user); // { name: 'Alice', age: 30 }

// Queue operations
await synap.queue.createQueue('jobs');
const msgId = await synap.queue.publishString('jobs', 'process-video');
const { message, text } = await synap.queue.consumeString('jobs', 'worker-1');
await synap.queue.ack('jobs', message.id);
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
import { SynapError, NetworkError, ServerError, TimeoutError } from '@synap/client';

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
import { Synap, QueueMessage, KVStats } from '@synap/client';

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
  import { Synap } from 'https://cdn.jsdelivr.net/npm/@synap/client/+esm';
  
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

# Run tests (requires running Synap server)
npm test

# Watch mode
npm run dev

# Lint
npm run lint

# Format
npm run format
```

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

