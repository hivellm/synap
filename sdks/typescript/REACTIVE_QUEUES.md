# Reactive Queue Patterns with RxJS

This document describes the reactive queue consumption patterns available in the Synap TypeScript SDK using RxJS.

## Why Reactive?

The traditional polling-based approach with while loops has several limitations:

- ❌ **Blocking**: Requires explicit polling and sleep intervals
- ❌ **Manual concurrency**: Difficult to process multiple messages in parallel
- ❌ **No composability**: Hard to combine with other async operations
- ❌ **Limited error handling**: Requires extensive try-catch blocks
- ❌ **No backpressure**: Can't easily control message flow

The reactive approach with RxJS provides:

- ✅ **Non-blocking**: Event-driven message consumption
- ✅ **Built-in concurrency**: Easy to configure parallel processing
- ✅ **Composability**: Rich operator library for stream manipulation
- ✅ **Declarative error handling**: Retry, catchError, and more
- ✅ **Backpressure control**: Built-in flow control mechanisms
- ✅ **Observability**: Easy to monitor and debug message flows

## Installation

The SDK includes RxJS as a dependency:

```bash
npm install @hivellm/synap
```

## Basic Reactive Consumer

### Simple Consumer with Manual ACK

```typescript
import { Synap } from '@hivellm/synap';

const synap = new Synap({ url: 'http://localhost:15500' });

// Subscribe to messages
synap.queue.consume$({
  queueName: 'tasks',
  consumerId: 'worker-1',
  pollingInterval: 500,
  concurrency: 5
}).subscribe({
  next: async (msg) => {
    console.log('Processing:', msg.data);
    
    try {
      // Process message
      await processTask(msg.data);
      
      // Acknowledge on success
      await msg.ack();
    } catch (error) {
      // Negative acknowledge on error (will retry)
      await msg.nack();
    }
  },
  error: (err) => console.error('Stream error:', err),
  complete: () => console.log('Stream completed')
});
```

### Auto-Processing with Handler Function

The `process$` method provides automatic ACK/NACK handling:

```typescript
synap.queue.process$({
  queueName: 'emails',
  consumerId: 'email-worker',
  pollingInterval: 500,
  concurrency: 10
}, async (data, message) => {
  // Process message - automatically ACK on success, NACK on error
  await sendEmail(data);
}).subscribe({
  next: (result) => {
    if (result.success) {
      console.log('✅ Processed:', result.messageId);
    } else {
      console.error('❌ Failed:', result.messageId, result.error);
    }
  }
});
```

## Configuration Options

### QueueConsumerOptions

```typescript
interface QueueConsumerOptions {
  queueName: string;       // Queue to consume from
  consumerId: string;      // Unique consumer identifier
  pollingInterval?: number; // Poll interval in ms (default: 1000)
  concurrency?: number;     // Max concurrent messages (default: 1)
  autoAck?: boolean;        // Auto-acknowledge (default: false)
  autoNack?: boolean;       // Auto-nack on error (default: false)
  requeueOnNack?: boolean;  // Requeue on nack (default: true)
}
```

## Advanced Patterns

### 1. Priority-Based Processing

Process only high-priority messages:

```typescript
import { filter } from 'rxjs/operators';

synap.queue.consume$({
  queueName: 'tasks',
  consumerId: 'priority-worker'
}).pipe(
  filter(msg => msg.message.priority >= 7)
).subscribe({
  next: async (msg) => {
    console.log('High-priority task:', msg.data);
    await processFast(msg.data);
    await msg.ack();
  }
});
```

### 2. Batch Processing

Collect and process messages in batches:

```typescript
import { bufferTime, filter } from 'rxjs/operators';

synap.queue.consume$({
  queueName: 'analytics',
  consumerId: 'batch-worker',
  pollingInterval: 100
}).pipe(
  bufferTime(5000),
  filter(batch => batch.length > 0)
).subscribe({
  next: async (batch) => {
    console.log(`Processing batch of ${batch.length} messages`);
    await processBatch(batch.map(m => m.data));
    await Promise.all(batch.map(m => m.ack()));
  }
});
```

### 3. Type-Based Routing

Route messages to different handlers based on type:

```typescript
const messages$ = synap.queue.consume$({
  queueName: 'tasks',
  consumerId: 'router'
});

// Email handler
messages$.pipe(
  filter(msg => msg.data.type === 'email')
).subscribe(async (msg) => {
  await sendEmail(msg.data);
  await msg.ack();
});

// Notification handler
messages$.pipe(
  filter(msg => msg.data.type === 'notification')
).subscribe(async (msg) => {
  await sendNotification(msg.data);
  await msg.ack();
});
```

### 4. Transform and Forward

Transform messages and publish to another queue:

```typescript
import { map } from 'rxjs/operators';

synap.queue.consume$({
  queueName: 'input',
  consumerId: 'transformer'
}).pipe(
  map(msg => ({
    original: msg,
    transformed: {
      ...msg.data,
      processed_at: new Date().toISOString()
    }
  }))
).subscribe({
  next: async ({ original, transformed }) => {
    await synap.queue.publishJSON('output', transformed);
    await original.ack();
  }
});
```

### 5. Retry with Exponential Backoff

```typescript
import { retry, catchError, debounceTime } from 'rxjs/operators';
import { of } from 'rxjs';

synap.queue.consume$({
  queueName: 'tasks',
  consumerId: 'retry-worker'
}).pipe(
  mergeMap(async (msg) => {
    // Might fail
    await riskyOperation(msg.data);
    return msg;
  }),
  retry({
    count: 3,
    delay: (error, retryCount) => {
      const delay = Math.pow(2, retryCount) * 1000;
      return of(error).pipe(debounceTime(delay));
    }
  }),
  catchError((error, caught) => {
    console.error('All retries exhausted:', error);
    return of(null);
  })
).subscribe({
  next: async (msg) => {
    if (msg) await msg.ack();
  }
});
```

### 6. Multi-Queue Monitoring

Monitor multiple queues simultaneously:

```typescript
import { combineLatest } from 'rxjs';

combineLatest([
  synap.queue.stats$('queue-a', 3000),
  synap.queue.stats$('queue-b', 3000),
  synap.queue.stats$('queue-c', 3000)
]).subscribe(([statsA, statsB, statsC]) => {
  console.log('Multi-Queue Stats:', {
    queueA: { depth: statsA.depth, acked: statsA.acked },
    queueB: { depth: statsB.depth, acked: statsB.acked },
    queueC: { depth: statsC.depth, acked: statsC.acked }
  });
});
```

## Graceful Shutdown

Properly stop consumers on shutdown:

```typescript
const subscription = synap.queue.process$({
  queueName: 'tasks',
  consumerId: 'worker-1',
  concurrency: 5
}, async (data) => {
  await processTask(data);
}).subscribe();

// Handle shutdown signals
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

## API Reference

### consume$\<T\>(options: QueueConsumerOptions): Observable\<ProcessedMessage\<T\>\>

Creates a reactive message consumer that emits `ProcessedMessage` objects.

**Returns:** `ProcessedMessage<T>` with:
- `message`: Original queue message
- `data`: Decoded payload
- `ack()`: Function to acknowledge message
- `nack(requeue?)`: Function to negative acknowledge

### process$\<T\>(options: QueueConsumerOptions, handler: Function): Observable\<Result\>

Creates a consumer with automatic ACK/NACK handling.

**Parameters:**
- `options`: Consumer configuration
- `handler`: `async (data: T, message: QueueMessage) => Promise<void>`

**Returns:** Observable of processing results with:
- `messageId`: Message ID
- `success`: Whether processing succeeded
- `error?`: Error if processing failed

### stats$(queueName: string, interval?: number): Observable\<QueueStats\>

Creates an observable that emits queue statistics at regular intervals.

**Parameters:**
- `queueName`: Queue to monitor
- `interval`: Polling interval in ms (default: 5000)

**Returns:** Observable of `QueueStats`

### stopConsumer(queueName: string, consumerId: string): void

Stops a specific reactive consumer.

### stopAllConsumers(): void

Stops all reactive consumers.

## Examples

See the `examples/` directory for complete working examples:

- `queue-worker.ts` - Production-ready reactive worker
- `reactive-patterns.ts` - Advanced patterns collection
- `basic-usage.ts` - Basic reactive usage

Run examples:

```bash
# Reactive worker
npm run build && node dist/examples/queue-worker.js

# Advanced patterns
npm run build && node dist/examples/reactive-patterns.js [1-7]
```

## Performance Considerations

1. **Concurrency**: Use `concurrency` option to process multiple messages in parallel
2. **Polling Interval**: Lower intervals = more responsive, higher CPU usage
3. **Batch Processing**: For high throughput, use `bufferTime` to process batches
4. **Backpressure**: Use RxJS operators like `throttleTime` or `debounceTime` to control flow
5. **Memory**: Unsubscribe when done to prevent memory leaks

## Comparison: While Loop vs Reactive

### ❌ Old Way (While Loop)

```typescript
while (running) {
  const { message, data } = await synap.queue.consumeJSON('tasks', 'worker-1');
  
  if (!message) {
    await sleep(1000);
    continue;
  }
  
  try {
    await processTask(data);
    await synap.queue.ack('tasks', message.id);
  } catch (error) {
    await synap.queue.nack('tasks', message.id);
  }
}
```

### ✅ New Way (Reactive)

```typescript
synap.queue.process$({
  queueName: 'tasks',
  consumerId: 'worker-1',
  pollingInterval: 500,
  concurrency: 5
}, async (data) => {
  await processTask(data);
}).subscribe();
```

## Benefits Summary

| Feature | While Loop | Reactive |
|---------|-----------|----------|
| Concurrency | Manual | Built-in |
| Error Handling | Try-catch | Operators |
| Backpressure | Manual | Built-in |
| Composability | Low | High |
| Code Lines | More | Less |
| Observability | Manual | Built-in |
| Testing | Harder | Easier |

## License

MIT - See LICENSE file for details.

