/**
 * Reactive Queue Worker Example
 * 
 * Demonstrates a production-ready reactive queue worker pattern using RxJS
 */

import { Synap } from '../src/index';
import { finalize, tap, catchError } from 'rxjs/operators';
import { of } from 'rxjs';

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

async function reactiveWorker() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  const QUEUE_NAME = 'email-queue';
  const WORKER_ID = `worker-${process.pid}`;

  console.log(`🚀 Starting reactive worker: ${WORKER_ID}`);

  // Create queue if it doesn't exist
  await synap.queue.createQueue(QUEUE_NAME, {
    max_depth: 10000,
    ack_deadline_secs: 30,
    default_max_retries: 3,
  });

  let processed = 0;
  let failed = 0;

  // Start consuming messages reactively with concurrency
  const subscription = synap.queue.processMessages<EmailTask>(
    {
      queueName: QUEUE_NAME,
      consumerId: WORKER_ID,
      pollingInterval: 500,  // Poll every 500ms
      concurrency: 5,        // Process up to 5 messages concurrently
    },
    async (data, message) => {
      console.log(`\n📨 Processing message: ${message.id}`);
      console.log(`   Priority: ${message.priority}, Retry: ${message.retry_count}/${message.max_retries}`);
      
      await processEmail(data);
    }
  ).pipe(
    tap((result) => {
      if (result.success) {
        processed++;
        console.log(`✅ Message ${result.messageId} processed (total: ${processed})`);
      } else {
        failed++;
        console.error(`❌ Message ${result.messageId} failed (total failed: ${failed}):`, result.error);
      }
    }),
    catchError((error) => {
      console.error('Worker error:', error);
      return of({ messageId: 'unknown', success: false, error });
    }),
    finalize(async () => {
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
    })
  ).subscribe({
    error: (err) => {
      console.error('Fatal error:', err);
      process.exit(1);
    }
  });

  // Monitor queue stats every 5 seconds
  const statsSubscription = synap.queue.observeStats(QUEUE_NAME, 5000)
    .subscribe({
      next: (stats) => {
        console.log(`\n📊 Queue Stats - Depth: ${stats.depth}, Consumers: ${stats.consumers}, Acked: ${stats.acked}, Nacked: ${stats.nacked}`);
      },
      error: (err) => console.error('Stats error:', err)
    });

  // Graceful shutdown
  process.on('SIGINT', () => {
    console.log('\n⚠️  Shutting down gracefully...');
    
    // Stop consuming new messages
    synap.queue.stopConsumer(QUEUE_NAME, WORKER_ID);
    
    // Unsubscribe from stats
    statsSubscription.unsubscribe();
    
    // Wait a bit for current messages to finish, then force stop
    setTimeout(() => {
      subscription.unsubscribe();
      process.exit(0);
    }, 2000);
  });

  process.on('SIGTERM', () => {
    console.log('\n⚠️  Received SIGTERM, shutting down...');
    synap.queue.stopConsumer(QUEUE_NAME, WORKER_ID);
    statsSubscription.unsubscribe();
    setTimeout(() => {
      subscription.unsubscribe();
      process.exit(0);
    }, 2000);
  });
}

// Run worker
reactiveWorker().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});

