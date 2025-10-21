/**
 * Basic Usage Example
 * 
 * Demonstrates basic operations with Synap SDK
 */

import { Synap } from '../src/index';

async function main() {
  // Create client
  const synap = new Synap({
    url: 'http://localhost:15500',
    debug: true,
  });

  console.log('=== Synap TypeScript SDK - Basic Usage ===\n');

  // 1. Health Check
  const health = await synap.health();
  console.log('✅ Server health:', health);

  // 2. Key-Value Operations
  console.log('\n=== Key-Value Operations ===');
  
  await synap.kv.set('user:1', { name: 'Alice', age: 30, role: 'admin' });
  console.log('✅ Set user:1');

  const user = await synap.kv.get('user:1');
  console.log('✅ Get user:1:', user);

  await synap.kv.set('counter', 0);
  await synap.kv.incr('counter', 10);
  const count = await synap.kv.get<number>('counter');
  console.log('✅ Counter:', count);

  // 3. Batch Operations
  console.log('\n=== Batch Operations ===');
  
  await synap.kv.mset({
    'product:1': { name: 'Widget', price: 9.99 },
    'product:2': { name: 'Gadget', price: 19.99 },
    'product:3': { name: 'Doohickey', price: 29.99 },
  });
  console.log('✅ MSET 3 products');

  const products = await synap.kv.mget(['product:1', 'product:2', 'product:3']);
  console.log('✅ MGET products:', Object.keys(products).length, 'items');

  // 4. Queue Operations
  console.log('\n=== Queue Operations ===');
  
  await synap.queue.createQueue('demo-queue');
  console.log('✅ Created queue: demo-queue');

  const msgId1 = await synap.queue.publishString('demo-queue', 'Task 1', { priority: 5 });
  console.log('✅ Published message:', msgId1);

  const msgId2 = await synap.queue.publishJSON('demo-queue', {
    task: 'send-email',
    to: 'user@example.com',
  });
  console.log('✅ Published JSON message:', msgId2);

  const { message, text } = await synap.queue.consumeString('demo-queue', 'worker-1');
  if (message) {
    console.log('✅ Consumed message:', text);
    console.log('   Priority:', message.priority);
    console.log('   Retry count:', message.retry_count);
    
    await synap.queue.ack('demo-queue', message.id);
    console.log('✅ ACK message');
  }

  // 5. Statistics
  console.log('\n=== Statistics ===');
  
  const kvStats = await synap.kv.stats();
  console.log('✅ KV Stats:', {
    keys: kvStats.total_keys,
    hit_rate: kvStats.hit_rate,
  });

  const queueStats = await synap.queue.stats('demo-queue');
  console.log('✅ Queue Stats:', {
    depth: queueStats.depth,
    published: queueStats.published,
  });

  // Cleanup
  await synap.queue.purge('demo-queue');
  await synap.queue.deleteQueue('demo-queue');
  console.log('\n✅ Cleanup complete');

  synap.close();
}

main().catch(console.error);

