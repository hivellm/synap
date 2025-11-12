/**
 * Queue Operations Examples
 * 
 * Demonstrates message queue operations: CREATE, PUBLISH, CONSUME, ACK, STATS, LIST
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runQueueExamples() {
  console.log('📨 === QUEUE OPERATIONS EXAMPLES ===\n');

  try {
    // CREATE QUEUE
    await synap.queue.createQueue('job-queue', {
      max_depth: 1000,
      ack_deadline_secs: 300,
    });
    console.log('✅ CREATE QUEUE');

    // PUBLISH (queue.publish expects string or Uint8Array)
    const msgId1 = await synap.queue.publish('job-queue', JSON.stringify({ type: 'email', to: 'user@example.com' }));
    const msgId2 = await synap.queue.publish('job-queue', JSON.stringify({ type: 'sms', to: '+1234567890' }));
    console.log('✅ PUBLISH messages:', msgId1, msgId2);

    // CONSUME
    const message = await synap.queue.consume('job-queue', 'worker-1');
    if (message) {
      console.log('✅ CONSUME message:', message.id);
      
      // ACK
      await synap.queue.ack('job-queue', message.id);
      console.log('✅ ACK message');
    }

    // STATS
    const queueStats = await synap.queue.stats('job-queue');
    console.log('✅ Queue Stats:', queueStats);

    // LIST QUEUES
    const queues = await synap.queue.listQueues();
    console.log('✅ LIST QUEUES:', queues);

    console.log('\n✅ Queue operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runQueueExamples().catch(console.error);

