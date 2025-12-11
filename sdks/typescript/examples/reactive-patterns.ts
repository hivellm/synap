/**
 * Advanced Reactive Queue Patterns
 * 
 * Demonstrates advanced patterns using RxJS operators for queue processing
 */

import { Synap } from '../src/index';
import { 
  filter, 
  map, 
  bufferTime, 
  mergeMap,
  retry,
  catchError,
  tap,
  debounceTime
} from 'rxjs/operators';
import { of, combineLatest } from 'rxjs';

interface Task {
  type: string;
  priority: number;
  data: any;
}

/**
 * Example 1: High-priority message processing
 */
async function priorityProcessing() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  console.log('📌 Pattern 1: Priority-based processing');

  await synap.queue.createQueue('tasks', {
    max_depth: 10000,
    default_max_retries: 3,
  });

  // Process only high-priority messages (priority >= 7)
  synap.queue.observeMessages<Task>({
    queueName: 'tasks',
    consumerId: 'priority-worker',
    pollingInterval: 500,
  }).pipe(
    filter(msg => msg.message.priority >= 7),
    tap(msg => console.log(`Processing HIGH priority task: ${msg.data.type}`))
  ).subscribe({
    next: async (msg) => {
      // Process high-priority task
      console.log(`  ⚡ Fast processing: ${msg.data.type}`);
      await msg.ack();
    },
    error: (err) => console.error('Error:', err)
  });
}

/**
 * Example 2: Batch processing
 */
async function batchProcessing() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  console.log('📦 Pattern 2: Batch processing (every 5 seconds)');

  await synap.queue.createQueue('batch-tasks', {
    max_depth: 10000,
  });

  // Collect messages and process in batches
  synap.queue.observeMessages<Task>({
    queueName: 'batch-tasks',
    consumerId: 'batch-worker',
    pollingInterval: 100,
  }).pipe(
    bufferTime(5000),  // Collect messages for 5 seconds
    filter(batch => batch.length > 0),
    tap(batch => console.log(`Processing batch of ${batch.length} messages`))
  ).subscribe({
    next: async (batch) => {
      // Process entire batch
      console.log(`  📦 Batch size: ${batch.length}`);
      
      // Acknowledge all messages in batch
      await Promise.all(batch.map(msg => msg.ack()));
    },
    error: (err) => console.error('Error:', err)
  });
}

/**
 * Example 3: Type-based routing
 */
async function typeBasedRouting() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  console.log('🔀 Pattern 3: Type-based message routing');

  await synap.queue.createQueue('mixed-tasks', {
    max_depth: 10000,
  });

  const messages$ = synap.queue.observeMessages<Task>({
    queueName: 'mixed-tasks',
    consumerId: 'router-worker',
    pollingInterval: 500,
  });

  // Route email tasks
  messages$.pipe(
    filter(msg => msg.data.type === 'email')
  ).subscribe({
    next: async (msg) => {
      console.log('📧 Sending email...');
      await msg.ack();
    }
  });

  // Route notification tasks
  messages$.pipe(
    filter(msg => msg.data.type === 'notification')
  ).subscribe({
    next: async (msg) => {
      console.log('🔔 Sending notification...');
      await msg.ack();
    }
  });

  // Route analytics tasks
  messages$.pipe(
    filter(msg => msg.data.type === 'analytics')
  ).subscribe({
    next: async (msg) => {
      console.log('📊 Processing analytics...');
      await msg.ack();
    }
  });
}

/**
 * Example 4: Retry with exponential backoff
 */
async function retryWithBackoff() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  console.log('🔄 Pattern 4: Retry with exponential backoff');

  await synap.queue.createQueue('retry-tasks', {
    max_depth: 10000,
    default_max_retries: 5,
  });

  synap.queue.observeMessages<Task>({
    queueName: 'retry-tasks',
    consumerId: 'retry-worker',
  }).pipe(
    mergeMap(async (msg) => {
      // Simulate processing that might fail
      if (Math.random() > 0.7) {
        throw new Error('Processing failed');
      }
      return msg;
    }),
    retry({
      count: 3,
      delay: (error, retryCount) => {
        const delay = Math.pow(2, retryCount) * 1000;
        console.log(`  ⏱️  Retry ${retryCount} after ${delay}ms`);
        return of(error).pipe(
          debounceTime(delay)
        );
      }
    }),
    catchError((error, caught) => {
      console.error('  ❌ All retries exhausted:', error.message);
      return of(null);
    })
  ).subscribe({
    next: async (msg) => {
      if (msg) {
        console.log('  ✅ Message processed successfully');
        await msg.ack();
      }
    }
  });
}

/**
 * Example 5: Multi-queue monitoring
 */
async function multiQueueMonitoring() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  console.log('👁️  Pattern 5: Multi-queue monitoring');

  await synap.queue.createQueue('queue-a', {});
  await synap.queue.createQueue('queue-b', {});
  await synap.queue.createQueue('queue-c', {});

  // Monitor multiple queues simultaneously
  combineLatest([
    synap.queue.observeStats('queue-a', 3000),
    synap.queue.observeStats('queue-b', 3000),
    synap.queue.observeStats('queue-c', 3000),
  ]).subscribe({
    next: ([statsA, statsB, statsC]) => {
      console.log('\n📊 Multi-Queue Stats:');
      console.log(`  Queue A - Depth: ${statsA.depth}, Acked: ${statsA.acked}`);
      console.log(`  Queue B - Depth: ${statsB.depth}, Acked: ${statsB.acked}`);
      console.log(`  Queue C - Depth: ${statsC.depth}, Acked: ${statsC.acked}`);
    }
  });
}

/**
 * Example 6: Transform and forward
 */
async function transformAndForward() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  console.log('🔀 Pattern 6: Transform and forward');

  await synap.queue.createQueue('input-queue', {});
  await synap.queue.createQueue('output-queue', {});

  // Consume from one queue, transform, and publish to another
  synap.queue.observeMessages<Task>({
    queueName: 'input-queue',
    consumerId: 'transformer',
    pollingInterval: 500,
  }).pipe(
    map(msg => ({
      original: msg,
      transformed: {
        ...msg.data,
        processed_at: new Date().toISOString(),
        transformed: true
      }
    }))
  ).subscribe({
    next: async ({ original, transformed }) => {
      console.log(`🔄 Transforming message: ${original.message.id}`);
      
      // Publish transformed message to output queue
      await synap.queue.publishJSON('output-queue', transformed, {
        priority: original.message.priority
      });
      
      // ACK original message
      await original.ack();
      
      console.log('  ✅ Transformed and forwarded');
    }
  });
}

/**
 * Example 7: Dead Letter Queue processor
 */
async function dlqProcessor() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  console.log('⚰️  Pattern 7: Dead Letter Queue processing');

  await synap.queue.createQueue('main-queue', {
    default_max_retries: 3,
  });
  await synap.queue.createQueue('dlq', {});

  // Monitor main queue stats for dead lettered messages
  synap.queue.observeStats('main-queue', 2000).pipe(
    filter(stats => stats.dead_lettered > 0),
    tap(stats => console.log(`⚠️  Dead lettered messages: ${stats.dead_lettered}`))
  ).subscribe();

  // Process DLQ messages differently
  synap.queue.observeMessages<Task>({
    queueName: 'dlq',
    consumerId: 'dlq-processor',
    pollingInterval: 5000,
  }).subscribe({
    next: async (msg) => {
      console.log('⚰️  Processing dead lettered message');
      console.log(`   Retries: ${msg.message.retry_count}/${msg.message.max_retries}`);
      
      // Log to external system, send alert, etc.
      console.log('   📝 Logging to monitoring system...');
      
      await msg.ack();
    }
  });
}

// Run examples based on command line argument
const pattern = process.argv[2] || '1';

switch (pattern) {
  case '1':
    priorityProcessing().catch(console.error);
    break;
  case '2':
    batchProcessing().catch(console.error);
    break;
  case '3':
    typeBasedRouting().catch(console.error);
    break;
  case '4':
    retryWithBackoff().catch(console.error);
    break;
  case '5':
    multiQueueMonitoring().catch(console.error);
    break;
  case '6':
    transformAndForward().catch(console.error);
    break;
  case '7':
    dlqProcessor().catch(console.error);
    break;
  default:
    console.log('Usage: ts-node reactive-patterns.ts [1-7]');
    console.log('  1 - Priority processing');
    console.log('  2 - Batch processing');
    console.log('  3 - Type-based routing');
    console.log('  4 - Retry with backoff');
    console.log('  5 - Multi-queue monitoring');
    console.log('  6 - Transform and forward');
    console.log('  7 - DLQ processor');
}

