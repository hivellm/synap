# TypeScript SDK Documentation

## Overview

The Synap TypeScript SDK provides a type-safe, Promise-based client for Node.js and browser environments.

## Installation

```bash
npm install @hivehub/synap
# or
pnpm add @hivehub/synap
# or
yarn add @hivehub/synap
```

## Quick Start

```typescript
import { Synap } from '@hivehub/synap';

const client = new SynapClient({
  url: 'http://localhost:15500',
  apiKey: 'synap_your_api_key'
});

// Key-Value operations
await client.kv.set('user:1', { name: 'Alice' }, 3600);
const value = await client.kv.get('user:1');

// Queue operations
await client.queue.publish('tasks', { type: 'send_email' });
const message = await client.queue.consume('tasks');
await client.queue.ack('tasks', message.messageId);

// Event streaming
await client.stream.subscribe('chat-room-1', (event) => {
  console.log('Event:', event);
});

// Pub/Sub
await client.pubsub.subscribe(['notifications.*'], (topic, message) => {
  console.log(`[${topic}]`, message);
});
```

## Client Configuration

```typescript
interface SynapClientConfig {
  url: string;               // Server URL
  apiKey?: string;           // API key for authentication
  timeout?: number;          // Request timeout (ms, default: 30000)
  retries?: number;          // Retry attempts (default: 3)
  poolSize?: number;         // Connection pool size (default: 10)
  keepAlive?: boolean;       // Keep-alive (default: true)
  compression?: boolean;     // Enable gzip (default: false)
  format?: 'json' | 'msgpack'; // Serialization format
}
```

### Example

```typescript
const client = new SynapClient({
  url: 'http://synap.example.com:15500',
  apiKey: process.env.SYNAP_API_KEY,
  timeout: 60000,
  retries: 5,
  poolSize: 20,
  compression: true,
  format: 'json'
});
```

## Key-Value API

### SET - Store Value

```typescript
await client.kv.set(
  key: string,
  value: any,
  ttl?: number,
  options?: {
    nx?: boolean,  // Only if not exists
    xx?: boolean   // Only if exists
  }
): Promise<SetResult>

interface SetResult {
  key: string;
  success: boolean;
  previous?: any;  // Previous value if xx=true
}
```

**Example**:
```typescript
// Simple set
await client.kv.set('session:abc', { userId: 123 });

// With TTL (1 hour)
await client.kv.set('cache:data', result, 3600);

// Set only if not exists
await client.kv.set('lock:resource', true, 60, { nx: true });
```

### GET - Retrieve Value

```typescript
await client.kv.get(key: string): Promise<GetResult>

interface GetResult {
  found: boolean;
  value?: any;
  ttl?: number;  // Remaining TTL in seconds
}
```

**Example**:
```typescript
const result = await client.kv.get('user:1001');

if (result.found) {
  console.log('Value:', result.value);
  console.log('TTL:', result.ttl);
} else {
  console.log('Key not found');
}
```

### DEL - Delete Keys

```typescript
await client.kv.del(...keys: string[]): Promise<number>
```

**Example**:
```typescript
// Delete single key
await client.kv.del('user:1001');

// Delete multiple keys
const deleted = await client.kv.del('user:1001', 'user:1002', 'user:1003');
console.log(`Deleted ${deleted} keys`);
```

### INCR/DECR - Atomic Increment

```typescript
await client.kv.incr(key: string, amount?: number): Promise<number>
await client.kv.decr(key: string, amount?: number): Promise<number>
```

**Example**:
```typescript
// Increment page views
const views = await client.kv.incr('article:123:views');

// Increment by custom amount
const count = await client.kv.incr('counter', 5);

// Decrement
const remaining = await client.kv.decr('inventory:item:42');
```

### SCAN - Scan Keys

```typescript
await client.kv.scan(
  prefix?: string,
  options?: {
    cursor?: string,
    count?: number
  }
): Promise<ScanResult>

interface ScanResult {
  keys: string[];
  cursor?: string;
  hasMore: boolean;
}
```

**Example**:
```typescript
// Scan all user keys
let cursor: string | undefined;
const allKeys: string[] = [];

do {
  const result = await client.kv.scan('user:', { cursor, count: 100 });
  allKeys.push(...result.keys);
  cursor = result.cursor;
} while (result.hasMore);
```

## Queue API

### PUBLISH - Add Message

```typescript
await client.queue.publish(
  queue: string,
  message: any,
  options?: {
    priority?: number,    // 0-9
    headers?: Record<string, string>
  }
): Promise<PublishResult>

interface PublishResult {
  messageId: string;
  position: number;
}
```

**Example**:
```typescript
const result = await client.queue.publish('tasks', {
  type: 'send_email',
  to: 'user@example.com',
  subject: 'Welcome!'
}, {
  priority: 8,
  headers: { source: 'signup-flow' }
});

console.log('Message ID:', result.messageId);
```

### CONSUME - Get Message

```typescript
await client.queue.consume(
  queue: string,
  options?: {
    timeout?: number,      // Seconds to wait
    ackDeadline?: number   // ACK deadline in seconds
  }
): Promise<QueueMessage | null>

interface QueueMessage {
  messageId: string;
  message: any;
  priority: number;
  retryCount: number;
  headers: Record<string, string>;
}
```

**Example**:
```typescript
// Poll for message (return immediately if none)
const msg = await client.queue.consume('tasks');

// Wait up to 30 seconds for message
const msg = await client.queue.consume('tasks', { timeout: 30 });

if (msg) {
  try {
    // Process message
    await processTask(msg.message);
    
    // Acknowledge success
    await client.queue.ack('tasks', msg.messageId);
  } catch (error) {
    // Negative acknowledge (requeue)
    await client.queue.nack('tasks', msg.messageId, true);
  }
}
```

### ACK - Acknowledge Message

```typescript
await client.queue.ack(
  queue: string,
  messageId: string
): Promise<void>
```

### NACK - Negative Acknowledge

```typescript
await client.queue.nack(
  queue: string,
  messageId: string,
  requeue?: boolean
): Promise<{ action: 'requeued' | 'dead_lettered' }>
```

## Event Stream API

### PUBLISH - Publish Event

```typescript
await client.stream.publish(
  room: string,
  eventType: string,
  data: any,
  metadata?: Record<string, string>
): Promise<PublishEventResult>

interface PublishEventResult {
  eventId: string;
  offset: number;
  subscribersNotified: number;
}
```

**Example**:
```typescript
const result = await client.stream.publish(
  'chat-room-1',
  'message',
  {
    user: 'alice',
    text: 'Hello everyone!',
    timestamp: new Date()
  }
);

console.log(`Event ${result.offset} delivered to ${result.subscribersNotified} subscribers`);
```

### SUBSCRIBE - Subscribe to Room

```typescript
await client.stream.subscribe(
  room: string,
  callback: (event: StreamEvent) => void,
  options?: {
    fromOffset?: number,
    replay?: boolean
  }
): Promise<Subscription>

interface StreamEvent {
  eventId: string;
  offset: number;
  eventType: string;
  data: any;
  timestamp: number;
}

interface Subscription {
  unsubscribe(): Promise<void>;
  room: string;
}
```

**Example**:
```typescript
// Subscribe to chat room
const subscription = await client.stream.subscribe(
  'chat-room-1',
  (event) => {
    if (event.eventType === 'message') {
      console.log(`${event.data.user}: ${event.data.text}`);
    }
  },
  {
    fromOffset: -50,  // Last 50 events
    replay: true
  }
);

// Later: unsubscribe
await subscription.unsubscribe();
```

### HISTORY - Get Event History

```typescript
await client.stream.history(
  room: string,
  options?: {
    fromOffset?: number,
    toOffset?: number,
    limit?: number
  }
): Promise<HistoryResult>

interface HistoryResult {
  events: StreamEvent[];
  oldestOffset: number;
  newestOffset: number;
}
```

**Example**:
```typescript
const history = await client.stream.history('chat-room-1', {
  fromOffset: 0,
  limit: 100
});

console.log(`Room has ${history.events.length} events`);
console.log(`Offset range: ${history.oldestOffset} - ${history.newestOffset}`);
```

## Pub/Sub API

### PUBLISH - Publish Message

```typescript
await client.pubsub.publish(
  topic: string,
  message: any,
  metadata?: Record<string, string>
): Promise<PublishResult>

interface PublishResult {
  messageId: string;
  topic: string;
  subscribersMatched: number;
}
```

**Example**:
```typescript
await client.pubsub.publish(
  'notifications.email.user',
  {
    to: 'alice@example.com',
    subject: 'Welcome!',
    body: 'Thanks for signing up'
  }
);
```

### SUBSCRIBE - Subscribe to Topics

```typescript
await client.pubsub.subscribe(
  topics: string[],
  callback: (topic: string, message: any) => void
): Promise<Subscription>
```

**Example**:
```typescript
// Subscribe with wildcards
const subscription = await client.pubsub.subscribe(
  ['notifications.email.*', 'events.user.#'],
  (topic, message) => {
    console.log(`[${topic}]`, message);
  }
);

// Unsubscribe
await subscription.unsubscribe();
```

## Error Handling

### SynapError Class

```typescript
class SynapError extends Error {
  code: string;
  details?: object;
  httpStatus?: number;
  
  constructor(code: string, message: string, details?: object);
}
```

### Error Handling Example

```typescript
try {
  const value = await client.kv.get('nonexistent-key');
} catch (error) {
  if (error instanceof SynapError) {
    if (error.code === 'KEY_NOT_FOUND') {
      console.log('Key not found:', error.details);
    } else {
      console.error('Synap error:', error.code, error.message);
    }
  } else {
    console.error('Unknown error:', error);
  }
}
```

### Typed Errors

```typescript
// Specific error classes
class KeyNotFoundError extends SynapError {}
class QueueNotFoundError extends SynapError {}
class RateLimitError extends SynapError {}
class ConnectionError extends SynapError {}
```

## Advanced Features

### Connection Events

```typescript
client.on('connected', () => {
  console.log('Connected to Synap');
});

client.on('disconnected', (reason) => {
  console.log('Disconnected:', reason);
});

client.on('error', (error) => {
  console.error('Client error:', error);
});
```

### Retry Configuration

```typescript
const client = new SynapClient({
  url: 'http://localhost:15500',
  retries: 5,
  retryDelay: 1000,        // Initial delay (ms)
  retryBackoff: 2,         // Exponential backoff multiplier
  retryMaxDelay: 30000     // Max delay (ms)
});
```

### Request Timeout

```typescript
// Global timeout
const client = new SynapClient({
  timeout: 60000  // 60 seconds
});

// Per-request timeout
await client.kv.get('key', { timeout: 5000 });
```

### Batch Operations

```typescript
const results = await client.batch([
  { command: 'kv.set', payload: { key: 'user:1', value: 'Alice' } },
  { command: 'kv.set', payload: { key: 'user:2', value: 'Bob' } },
  { command: 'kv.get', payload: { key: 'user:1' } }
]);

console.log(results);
// [
//   { status: 'success', payload: { success: true } },
//   { status: 'success', payload: { success: true } },
//   { status: 'success', payload: { found: true, value: 'Alice' } }
// ]
```

## Type Definitions

### Client Types

```typescript
interface SynapClientConfig {
  url: string;
  apiKey?: string;
  timeout?: number;
  retries?: number;
  poolSize?: number;
  keepAlive?: boolean;
  compression?: boolean;
  format?: 'json' | 'msgpack';
}

interface SetOptions {
  ttl?: number;
  nx?: boolean;
  xx?: boolean;
}

interface ScanOptions {
  cursor?: string;
  count?: number;
}

interface QueueConsumeOptions {
  timeout?: number;
  ackDeadline?: number;
}

interface StreamSubscribeOptions {
  fromOffset?: number;
  replay?: boolean;
}
```

### Response Types

```typescript
interface SetResult {
  key: string;
  success: boolean;
  previous?: any;
}

interface GetResult {
  found: boolean;
  value?: any;
  ttl?: number;
}

interface QueueMessage {
  messageId: string;
  message: any;
  priority: number;
  retryCount: number;
  headers: Record<string, string>;
}

interface StreamEvent {
  eventId: string;
  offset: number;
  eventType: string;
  data: any;
  timestamp: number;
}

interface Subscription {
  unsubscribe(): Promise<void>;
  room?: string;
  topics?: string[];
}
```

## Implementation Architecture

### Module Structure

```
@hivehub/synap/
├── src/
│   ├── index.ts              # Main exports
│   ├── client.ts             # SynapClient class
│   ├── transports/
│   │   ├── http.ts           # HTTP transport
│   │   └── websocket.ts      # WebSocket transport
│   ├── api/
│   │   ├── kv.ts             # Key-value API
│   │   ├── queue.ts          # Queue API
│   │   ├── stream.ts         # Event stream API
│   │   └── pubsub.ts         # Pub/sub API
│   ├── errors.ts             # Error classes
│   └── types.ts              # Type definitions
└── tests/
    ├── kv.test.ts
    ├── queue.test.ts
    ├── stream.test.ts
    └── pubsub.test.ts
```

### Transport Layer

```typescript
interface Transport {
  send(command: string, payload: any): Promise<any>;
  close(): Promise<void>;
}

class HTTPTransport implements Transport {
  private pool: ConnectionPool;
  
  async send(command: string, payload: any): Promise<any> {
    const envelope = {
      type: 'request',
      request_id: uuidv4(),
      command,
      version: '1.0',
      payload
    };
    
    const response = await this.pool.fetch('/api/v1/command', {
      method: 'POST',
      body: JSON.stringify(envelope)
    });
    
    return response.payload;
  }
}

class WebSocketTransport implements Transport {
  private ws: WebSocket;
  private pending: Map<string, Promise>;
  
  async send(command: string, payload: any): Promise<any> {
    const requestId = uuidv4();
    
    return new Promise((resolve, reject) => {
      this.pending.set(requestId, { resolve, reject });
      
      this.ws.send(JSON.stringify({
        request_id: requestId,
        command,
        payload
      }));
    });
  }
}
```

### Connection Pooling

```typescript
class ConnectionPool {
  private connections: Connection[];
  private available: Connection[];
  private maxSize: number;
  
  async acquire(): Promise<Connection> {
    if (this.available.length > 0) {
      return this.available.pop()!;
    }
    
    if (this.connections.length < this.maxSize) {
      const conn = await this.createConnection();
      this.connections.push(conn);
      return conn;
    }
    
    // Wait for available connection
    return this.waitForConnection();
  }
  
  release(conn: Connection): void {
    this.available.push(conn);
  }
}
```

## Examples

### Session Management

```typescript
class SessionManager {
  constructor(private client: SynapClient) {}
  
  async createSession(userId: number): Promise<string> {
    const sessionId = uuidv4();
    
    await this.client.kv.set(
      `session:${sessionId}`,
      { userId, createdAt: Date.now() },
      3600  // 1 hour TTL
    );
    
    return sessionId;
  }
  
  async getSession(sessionId: string): Promise<any | null> {
    const result = await this.client.kv.get(`session:${sessionId}`);
    return result.found ? result.value : null;
  }
  
  async destroySession(sessionId: string): Promise<void> {
    await this.client.kv.del(`session:${sessionId}`);
  }
}
```

### Task Queue Worker

```typescript
class TaskWorker {
  constructor(
    private client: SynapClient,
    private queueName: string
  ) {}
  
  async start(): Promise<void> {
    while (true) {
      try {
        // Wait up to 30 seconds for task
        const msg = await this.client.queue.consume(
          this.queueName,
          { timeout: 30, ackDeadline: 300 }
        );
        
        if (!msg) continue;
        
        // Process task
        await this.processTask(msg.message);
        
        // Acknowledge completion
        await this.client.queue.ack(this.queueName, msg.messageId);
        
      } catch (error) {
        console.error('Worker error:', error);
        
        // NACK on failure (will retry)
        if (msg) {
          await this.client.queue.nack(this.queueName, msg.messageId, true);
        }
      }
    }
  }
  
  private async processTask(task: any): Promise<void> {
    // Task processing logic
  }
}
```

### Real-Time Chat

```typescript
class ChatClient {
  private subscription?: Subscription;
  
  constructor(private client: SynapClient) {}
  
  async joinRoom(roomId: string, onMessage: (msg: any) => void): Promise<void> {
    this.subscription = await this.client.stream.subscribe(
      `chat-${roomId}`,
      (event) => {
        if (event.eventType === 'message') {
          onMessage(event.data);
        }
      },
      {
        fromOffset: -50,  // Get last 50 messages
        replay: true
      }
    );
  }
  
  async sendMessage(roomId: string, text: string): Promise<void> {
    await this.client.stream.publish(
      `chat-${roomId}`,
      'message',
      {
        user: this.getUserId(),
        text,
        timestamp: Date.now()
      }
    );
  }
  
  async leaveRoom(): Promise<void> {
    if (this.subscription) {
      await this.subscription.unsubscribe();
    }
  }
}
```

### Notification System

```typescript
class NotificationService {
  constructor(private client: SynapClient) {
    this.setupSubscriptions();
  }
  
  private async setupSubscriptions(): Promise<void> {
    // Subscribe to all notification types
    await this.client.pubsub.subscribe(
      ['notifications.#'],
      (topic, message) => {
        this.handleNotification(topic, message);
      }
    );
  }
  
  async sendEmail(to: string, subject: string): Promise<void> {
    await this.client.pubsub.publish(
      'notifications.email.user',
      { to, subject, timestamp: Date.now() }
    );
  }
  
  async sendSMS(to: string, text: string): Promise<void> {
    await this.client.pubsub.publish(
      'notifications.sms.user',
      { to, text }
    );
  }
  
  private handleNotification(topic: string, message: any): void {
    console.log(`Notification [${topic}]:`, message);
  }
}
```

## Testing

### Mock Client

```typescript
import { createMockClient } from '@hivehub/synap/testing';

const mockClient = createMockClient();

// Setup mock responses
mockClient.kv.get.mockResolvedValue({
  found: true,
  value: { name: 'Alice' }
});

// Use in tests
const result = await mockClient.kv.get('user:1');
expect(result.value.name).toBe('Alice');
```

### Integration Tests

```typescript
import { Synap } from '@hivehub/synap';

describe('Synap Integration', () => {
  let client: SynapClient;
  
  beforeAll(async () => {
    client = new SynapClient({
      url: 'http://localhost:15500',
      apiKey: 'test_key'
    });
  });
  
  afterAll(async () => {
    await client.close();
  });
  
  test('should set and get value', async () => {
    await client.kv.set('test-key', 'test-value');
    const result = await client.kv.get('test-key');
    
    expect(result.found).toBe(true);
    expect(result.value).toBe('test-value');
  });
});
```

## See Also

- [REST_API.md](../api/REST_API.md) - Complete API reference
- [PYTHON.md](PYTHON.md) - Python SDK documentation
- [RUST.md](RUST.md) - Rust SDK documentation
- [CHAT_SAMPLE.md](../examples/CHAT_SAMPLE.md) - Full chat example

