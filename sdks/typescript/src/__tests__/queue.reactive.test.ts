/**
 * Reactive Queue System Module Tests
 * Tests for RxJS-based reactive queue patterns
 */

import { describe, it, expect, beforeAll, afterAll, vi, beforeEach } from 'vitest';
import { Synap } from '../index';
import { take, toArray, timeout, bufferTime, filter } from 'rxjs/operators';
import { firstValueFrom, lastValueFrom } from 'rxjs';

describe('QueueManager - Reactive Methods', () => {
  let synap: Synap;
  const testQueue = 'test_reactive_queue';

  beforeAll(async () => {
    synap = new Synap({
      url: process.env.SYNAP_URL || 'http://localhost:15500',
    });

    await synap.queue.createQueue(testQueue, {
      max_depth: 10000,
      ack_deadline_secs: 30,
      default_max_retries: 3,
    });
  });

  afterAll(async () => {
    synap.queue.stopAllConsumers();
    await synap.queue.purge(testQueue);
    await synap.queue.deleteQueue(testQueue);
    synap.close();
  });

  beforeEach(async () => {
    // Clear queue before each test
    await synap.queue.purge(testQueue);
  });

  describe('consume$() - Basic Reactive Consumer', () => {
    it('should consume messages as observables', async () => {
      // Publish test messages
      await synap.queue.publishJSON(testQueue, { task: 'task1' });
      await synap.queue.publishJSON(testQueue, { task: 'task2' });

      // Consume reactively
      const messages = await firstValueFrom(
        synap.queue.consume$<{ task: string }>({
          queueName: testQueue,
          consumerId: 'test-consumer-1',
          pollingInterval: 100,
        }).pipe(
          take(2),
          toArray(),
          timeout(5000)
        )
      );

      expect(messages).toHaveLength(2);
      expect(messages[0].data.task).toBe('task1');
      expect(messages[1].data.task).toBe('task2');
      expect(messages[0].message).toBeTruthy();
      expect(messages[0].message.id).toBeTruthy();

      // Clean up
      await messages[0].ack();
      await messages[1].ack();
      synap.queue.stopConsumer(testQueue, 'test-consumer-1');
    }, 10000);

    it('should provide ack() and nack() methods on messages', async () => {
      await synap.queue.publishJSON(testQueue, { task: 'ack-test' });

      const msg = await firstValueFrom(
        synap.queue.consume$<{ task: string }>({
          queueName: testQueue,
          consumerId: 'test-consumer-2',
        }).pipe(timeout(5000))
      );

      expect(msg.ack).toBeTypeOf('function');
      expect(msg.nack).toBeTypeOf('function');

      // Test ACK
      await expect(msg.ack()).resolves.not.toThrow();

      synap.queue.stopConsumer(testQueue, 'test-consumer-2');
    }, 10000);

    it('should support custom polling interval', async () => {
      await synap.queue.publishJSON(testQueue, { task: 'polling-test' });

      const startTime = Date.now();
      
      const msg = await firstValueFrom(
        synap.queue.consume$<{ task: string }>({
          queueName: testQueue,
          consumerId: 'test-consumer-3',
          pollingInterval: 50, // Very fast polling
        }).pipe(timeout(5000))
      );

      const elapsed = Date.now() - startTime;
      
      expect(msg).toBeTruthy();
      expect(elapsed).toBeLessThan(1000); // Should be fast
      
      await msg.ack();
      synap.queue.stopConsumer(testQueue, 'test-consumer-3');
    }, 10000);

    it('should handle concurrency correctly', async () => {
      // Publish multiple messages
      for (let i = 0; i < 5; i++) {
        await synap.queue.publishJSON(testQueue, { task: `task${i}` });
      }

      const messages = await firstValueFrom(
        synap.queue.consume$<{ task: string }>({
          queueName: testQueue,
          consumerId: 'test-consumer-4',
          pollingInterval: 100,
          concurrency: 3, // Process 3 at a time
        }).pipe(
          take(5),
          toArray(),
          timeout(10000)
        )
      );

      expect(messages).toHaveLength(5);
      
      // ACK all
      await Promise.all(messages.map(m => m.ack()));
      synap.queue.stopConsumer(testQueue, 'test-consumer-4');
    }, 15000);
  });

  describe('process$() - Auto-Processing Consumer', () => {
    it('should process messages with auto-ACK on success', async () => {
      await synap.queue.publishJSON(testQueue, { value: 10 });
      await synap.queue.publishJSON(testQueue, { value: 20 });

      const processedValues: number[] = [];
      
      const results = await firstValueFrom(
        synap.queue.process$<{ value: number }>(
          {
            queueName: testQueue,
            consumerId: 'test-processor-1',
            pollingInterval: 100,
          },
          async (data, message) => {
            processedValues.push(data.value);
          }
        ).pipe(
          take(2),
          toArray(),
          timeout(5000)
        )
      );

      expect(results).toHaveLength(2);
      expect(results[0].success).toBe(true);
      expect(results[1].success).toBe(true);
      expect(processedValues).toEqual([10, 20]);

      synap.queue.stopConsumer(testQueue, 'test-processor-1');
    }, 10000);

    it('should auto-NACK on processing error', async () => {
      await synap.queue.publishJSON(testQueue, { shouldFail: true });

      const results = await firstValueFrom(
        synap.queue.process$<{ shouldFail: boolean }>(
          {
            queueName: testQueue,
            consumerId: 'test-processor-2',
            pollingInterval: 100,
          },
          async (data) => {
            if (data.shouldFail) {
              throw new Error('Processing failed');
            }
          }
        ).pipe(
          take(1),
          timeout(5000)
        )
      );

      expect(results.success).toBe(false);
      expect(results.error).toBeTruthy();
      expect(results.error?.message).toBe('Processing failed');

      synap.queue.stopConsumer(testQueue, 'test-processor-2');
    }, 10000);

    it('should support concurrency in processing', async () => {
      // Publish messages
      for (let i = 0; i < 10; i++) {
        await synap.queue.publishJSON(testQueue, { id: i });
      }

      const processedIds: number[] = [];
      let concurrentCount = 0;
      let maxConcurrent = 0;

      const results = await firstValueFrom(
        synap.queue.process$<{ id: number }>(
          {
            queueName: testQueue,
            consumerId: 'test-processor-3',
            pollingInterval: 50,
            concurrency: 5,
          },
          async (data) => {
            concurrentCount++;
            maxConcurrent = Math.max(maxConcurrent, concurrentCount);
            
            // Simulate processing time
            await new Promise(resolve => setTimeout(resolve, 100));
            
            processedIds.push(data.id);
            concurrentCount--;
          }
        ).pipe(
          take(10),
          toArray(),
          timeout(10000)
        )
      );

      expect(results).toHaveLength(10);
      expect(processedIds).toHaveLength(10);
      expect(maxConcurrent).toBeGreaterThan(1); // Should process concurrently
      expect(maxConcurrent).toBeLessThanOrEqual(5); // Should not exceed concurrency limit

      synap.queue.stopConsumer(testQueue, 'test-processor-3');
    }, 15000);

    it('should provide message metadata to handler', async () => {
      await synap.queue.publishJSON(testQueue, { data: 'test' }, { 
        priority: 9,
      });

      let capturedMessage: any = null;

      await firstValueFrom(
        synap.queue.process$<{ data: string }>(
          {
            queueName: testQueue,
            consumerId: 'test-processor-4',
          },
          async (data, message) => {
            capturedMessage = message;
          }
        ).pipe(
          take(1),
          timeout(5000)
        )
      );

      expect(capturedMessage).toBeTruthy();
      expect(capturedMessage.priority).toBe(9);
      expect(capturedMessage.id).toBeTruthy();
      expect(capturedMessage.payload).toBeTruthy();
      expect(typeof capturedMessage.retry_count).toBe('number');

      synap.queue.stopConsumer(testQueue, 'test-processor-4');
    }, 10000);
  });

  describe('stats$() - Reactive Stats Monitoring', () => {
    it('should emit queue stats at regular intervals', async () => {
      // Publish some messages
      await synap.queue.publishJSON(testQueue, { data: 'msg1' });
      await synap.queue.publishJSON(testQueue, { data: 'msg2' });

      const stats = await firstValueFrom(
        synap.queue.stats$(testQueue, 500).pipe(
          take(2),
          toArray(),
          timeout(5000)
        )
      );

      expect(stats).toHaveLength(2);
      expect(stats[0]).toHaveProperty('depth');
      expect(stats[0]).toHaveProperty('published');
      expect(stats[0]).toHaveProperty('consumed');
      expect(typeof stats[0].depth).toBe('number');
    }, 10000);

    it('should reflect queue changes in stats', async () => {
      // Start monitoring
      const statsPromise = firstValueFrom(
        synap.queue.stats$(testQueue, 300).pipe(
          take(3),
          toArray(),
          timeout(5000)
        )
      );

      // Publish messages while monitoring
      await new Promise(resolve => setTimeout(resolve, 100));
      await synap.queue.publishJSON(testQueue, { data: 'test' });

      const stats = await statsPromise;
      
      expect(stats).toHaveLength(3);
      // Later stats should show increased published count
      const firstPublished = stats[0].published;
      const lastPublished = stats[2].published;
      expect(lastPublished).toBeGreaterThanOrEqual(firstPublished);
    }, 10000);
  });

  describe('stopConsumer() - Lifecycle Management', () => {
    it('should stop a specific consumer', async () => {
      // Publish messages
      for (let i = 0; i < 10; i++) {
        await synap.queue.publishJSON(testQueue, { id: i });
      }

      let consumedCount = 0;

      const subscription = synap.queue.consume$<{ id: number }>({
        queueName: testQueue,
        consumerId: 'test-stop-consumer',
        pollingInterval: 100,
      }).subscribe({
        next: async (msg) => {
          consumedCount++;
          await msg.ack();
        }
      });

      // Let it consume a few
      await new Promise(resolve => setTimeout(resolve, 500));

      // Stop consumer
      synap.queue.stopConsumer(testQueue, 'test-stop-consumer');
      
      const countAfterStop = consumedCount;
      
      // Wait a bit more
      await new Promise(resolve => setTimeout(resolve, 500));
      
      // Should not consume more after stop
      expect(consumedCount).toBe(countAfterStop);
      
      subscription.unsubscribe();
    }, 10000);

    it('should stop all consumers', async () => {
      for (let i = 0; i < 5; i++) {
        await synap.queue.publishJSON(testQueue, { id: i });
      }

      let count1 = 0;
      let count2 = 0;

      const sub1 = synap.queue.consume$({
        queueName: testQueue,
        consumerId: 'consumer-1',
      }).subscribe({ next: async (msg) => { count1++; await msg.ack(); }});

      const sub2 = synap.queue.consume$({
        queueName: testQueue,
        consumerId: 'consumer-2',
      }).subscribe({ next: async (msg) => { count2++; await msg.ack(); }});

      await new Promise(resolve => setTimeout(resolve, 500));

      // Stop all
      synap.queue.stopAllConsumers();

      const total1 = count1;
      const total2 = count2;

      await new Promise(resolve => setTimeout(resolve, 500));

      expect(count1).toBe(total1);
      expect(count2).toBe(total2);

      sub1.unsubscribe();
      sub2.unsubscribe();
    }, 10000);
  });

  describe('Advanced Reactive Patterns', () => {
    it('should support priority filtering', async () => {
      // Publish mixed priority messages
      await synap.queue.publishJSON(testQueue, { priority: 'low' }, { priority: 3 });
      await synap.queue.publishJSON(testQueue, { priority: 'high' }, { priority: 9 });
      await synap.queue.publishJSON(testQueue, { priority: 'medium' }, { priority: 5 });

      // Filter only high priority (>= 7)
      const highPriorityMessages = await firstValueFrom(
        synap.queue.consume$<{ priority: string }>({
          queueName: testQueue,
          consumerId: 'priority-filter',
          pollingInterval: 100,
        }).pipe(
          filter(msg => msg.message.priority >= 7),
          take(1),
          toArray(),
          timeout(5000)
        )
      );

      expect(highPriorityMessages).toHaveLength(1);
      expect(highPriorityMessages[0].data.priority).toBe('high');

      await highPriorityMessages[0].ack();
      
      // Clean up remaining messages
      const { message: msg1 } = await synap.queue.consumeJSON(testQueue, 'cleanup');
      const { message: msg2 } = await synap.queue.consumeJSON(testQueue, 'cleanup');
      if (msg1) await synap.queue.ack(testQueue, msg1.id);
      if (msg2) await synap.queue.ack(testQueue, msg2.id);

      synap.queue.stopConsumer(testQueue, 'priority-filter');
    }, 10000);

    it('should support batch processing with bufferTime', async () => {
      // Publish messages quickly
      for (let i = 0; i < 5; i++) {
        await synap.queue.publishJSON(testQueue, { id: i });
      }

      const batches = await firstValueFrom(
        synap.queue.consume$<{ id: number }>({
          queueName: testQueue,
          consumerId: 'batch-processor',
          pollingInterval: 50,
        }).pipe(
          bufferTime(1000),
          take(1),
          timeout(5000)
        )
      );

      expect(batches.length).toBeGreaterThan(0);
      
      // ACK all messages in batch
      await Promise.all(batches.map(msg => msg.ack()));

      synap.queue.stopConsumer(testQueue, 'batch-processor');
    }, 10000);

    it('should support type-based routing', async () => {
      await synap.queue.publishJSON(testQueue, { type: 'email', content: 'email1' });
      await synap.queue.publishJSON(testQueue, { type: 'sms', content: 'sms1' });
      await synap.queue.publishJSON(testQueue, { type: 'email', content: 'email2' });

      const messages$ = synap.queue.consume$<{ type: string; content: string }>({
        queueName: testQueue,
        consumerId: 'router',
        pollingInterval: 100,
      });

      // Route emails
      const emails = await firstValueFrom(
        messages$.pipe(
          filter(msg => msg.data.type === 'email'),
          take(2),
          toArray(),
          timeout(5000)
        )
      );

      expect(emails).toHaveLength(2);
      expect(emails[0].data.content).toBe('email1');
      expect(emails[1].data.content).toBe('email2');

      await Promise.all(emails.map(e => e.ack()));

      // Clean up SMS
      const { message: smsMsg } = await synap.queue.consumeJSON(testQueue, 'cleanup');
      if (smsMsg) await synap.queue.ack(testQueue, smsMsg.id);

      synap.queue.stopConsumer(testQueue, 'router');
    }, 10000);
  });

  describe('Error Handling', () => {
    it('should handle errors in consume$ gracefully', async () => {
      // Consumer should not crash on internal errors
      const messages: any[] = [];
      
      const subscription = synap.queue.consume$({
        queueName: testQueue,
        consumerId: 'error-handler',
        pollingInterval: 100,
      }).subscribe({
        next: (msg) => messages.push(msg),
        error: (err) => {
          // Should not error under normal conditions
          expect(err).toBeUndefined();
        }
      });

      await new Promise(resolve => setTimeout(resolve, 500));

      subscription.unsubscribe();
      synap.queue.stopConsumer(testQueue, 'error-handler');
    }, 5000);

    it('should continue consuming after handler errors in process$', async () => {
      await synap.queue.publishJSON(testQueue, { shouldFail: true });
      await synap.queue.publishJSON(testQueue, { shouldFail: false });
      await synap.queue.publishJSON(testQueue, { shouldFail: false });

      const results = await firstValueFrom(
        synap.queue.process$<{ shouldFail: boolean }>(
          {
            queueName: testQueue,
            consumerId: 'error-recovery',
            pollingInterval: 100,
          },
          async (data) => {
            if (data.shouldFail) {
              throw new Error('Intentional failure');
            }
          }
        ).pipe(
          take(3),
          toArray(),
          timeout(5000)
        )
      );

      expect(results).toHaveLength(3);
      expect(results[0].success).toBe(false); // First one fails
      expect(results[1].success).toBe(true);  // Second succeeds
      expect(results[2].success).toBe(true);  // Third succeeds

      synap.queue.stopConsumer(testQueue, 'error-recovery');
    }, 10000);
  });
});

