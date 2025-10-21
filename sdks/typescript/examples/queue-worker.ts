/**
 * Queue Worker Example
 * 
 * Demonstrates a production-ready queue worker pattern
 */

import { Synap } from '../src/index';

interface EmailTask {
  task: 'send-email';
  to: string;
  subject: string;
  body: string;
}

async function processEmail(task: EmailTask): Promise<void> {
  console.log(`📧 Sending email to ${task.to}...`);
  console.log(`   Subject: ${task.subject}`);
  
  // Simulate email sending
  await new Promise(resolve => setTimeout(resolve, 100));
  
  console.log('✅ Email sent successfully');
}

async function queueWorker() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  const QUEUE_NAME = 'email-queue';
  const WORKER_ID = `worker-${process.pid}`;

  console.log(`🚀 Starting worker: ${WORKER_ID}`);

  // Create queue if it doesn't exist
  await synap.queue.createQueue(QUEUE_NAME, {
    max_depth: 10000,
    ack_deadline_secs: 30,
    default_max_retries: 3,
  });

  let running = true;
  let processed = 0;
  let failed = 0;

  // Graceful shutdown
  process.on('SIGINT', () => {
    console.log('\n⚠️  Shutting down gracefully...');
    running = false;
  });

  // Worker loop
  while (running) {
    try {
      const { message, data } = await synap.queue.consumeJSON<EmailTask>(
        QUEUE_NAME,
        WORKER_ID
      );

      if (!message) {
        // Queue empty, wait a bit
        await new Promise(resolve => setTimeout(resolve, 1000));
        continue;
      }

      console.log(`\n📨 Processing message: ${message.id}`);
      console.log(`   Priority: ${message.priority}, Retry: ${message.retry_count}/${message.max_retries}`);

      try {
        // Process the task
        if (data) {
          await processEmail(data);
        }

        // ACK on success
        await synap.queue.ack(QUEUE_NAME, message.id);
        processed++;
        
        console.log(`✅ Message processed successfully (total: ${processed})`);
      } catch (error) {
        console.error(`❌ Processing failed:`, error);
        
        // NACK - will retry or go to DLQ
        await synap.queue.nack(QUEUE_NAME, message.id, true);
        failed++;
        
        console.log(`🔄 Message requeued for retry (failed: ${failed})`);
      }
    } catch (error) {
      console.error('Worker error:', error);
      await new Promise(resolve => setTimeout(resolve, 5000));
    }
  }

  // Print stats before exit
  const stats = await synap.queue.stats(QUEUE_NAME);
  console.log('\n📊 Final Stats:', {
    processed,
    failed,
    queueDepth: stats.depth,
    deadLettered: stats.dead_lettered,
  });

  synap.close();
  console.log('👋 Worker stopped');
}

// Run worker
queueWorker().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});

