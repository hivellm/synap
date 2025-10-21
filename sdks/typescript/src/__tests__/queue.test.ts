/**
 * Queue System Module Tests
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { Synap } from '../index';

describe('QueueManager', () => {
  let synap: Synap;
  const testQueue = 'test_queue_ts_sdk';

  beforeAll(async () => {
    synap = new Synap({
      url: process.env.SYNAP_URL || 'http://localhost:15500',
    });

    // Create test queue
    await synap.queue.createQueue(testQueue);
  });

  afterAll(async () => {
    // Cleanup
    await synap.queue.deleteQueue(testQueue);
    synap.close();
  });

  describe('Queue creation', () => {
    it('should create a queue with default config', async () => {
      const created = await synap.queue.createQueue('test_create_queue');
      expect(created).toBe(true);
      
      await synap.queue.deleteQueue('test_create_queue');
    });

    it('should create a queue with custom config', async () => {
      const created = await synap.queue.createQueue('test_custom_queue', {
        max_depth: 5000,
        ack_deadline_secs: 60,
        default_max_retries: 5,
        default_priority: 7,
      });
      expect(created).toBe(true);
      
      await synap.queue.deleteQueue('test_custom_queue');
    });
  });

  describe('Publish/Consume operations', () => {
    it('should publish and consume a string message', async () => {
      const messageId = await synap.queue.publishString(testQueue, 'Hello from TypeScript!');
      expect(messageId).toBeTruthy();

      const { message, text } = await synap.queue.consumeString(testQueue, 'ts-worker');
      expect(message).toBeTruthy();
      expect(text).toBe('Hello from TypeScript!');
      expect(message!.id).toBe(messageId);

      await synap.queue.ack(testQueue, message!.id);
    });

    it('should publish and consume JSON message', async () => {
      const data = { task: 'process-video', videoId: '12345', priority: 'high' };
      
      const messageId = await synap.queue.publishJSON(testQueue, data);
      expect(messageId).toBeTruthy();

      const { message, data: receivedData } = await synap.queue.consumeJSON<typeof data>(
        testQueue,
        'ts-worker'
      );

      expect(message).toBeTruthy();
      expect(receivedData).toEqual(data);

      await synap.queue.ack(testQueue, message!.id);
    });

    it('should publish with priority', async () => {
      await synap.queue.publishString(testQueue, 'Low priority', { priority: 1 });
      await synap.queue.publishString(testQueue, 'High priority', { priority: 9 });

      // High priority should come first
      const { text: first } = await synap.queue.consumeString(testQueue, 'ts-worker');
      expect(first).toBe('High priority');

      const { text: second } = await synap.queue.consumeString(testQueue, 'ts-worker');
      expect(second).toBe('Low priority');
    });

    it('should return null when consuming from empty queue', async () => {
      const emptyQueue = 'test_empty_queue';
      await synap.queue.createQueue(emptyQueue);

      const message = await synap.queue.consume(emptyQueue, 'ts-worker');
      expect(message).toBeNull();

      await synap.queue.deleteQueue(emptyQueue);
    });
  });

  describe('ACK/NACK operations', () => {
    it('should ACK a message', async () => {
      await synap.queue.publishString(testQueue, 'ACK test');
      const { message } = await synap.queue.consumeString(testQueue, 'ts-worker');

      const acked = await synap.queue.ack(testQueue, message!.id);
      expect(acked).toBe(true);
    });

    it('should NACK a message with requeue', async () => {
      await synap.queue.publishString(testQueue, 'NACK test', { max_retries: 3 });
      const { message } = await synap.queue.consumeString(testQueue, 'ts-worker');

      const nacked = await synap.queue.nack(testQueue, message!.id, true);
      expect(nacked).toBe(true);

      // Should be back in queue
      const { message: requeuedMsg } = await synap.queue.consumeString(testQueue, 'ts-worker');
      expect(requeuedMsg).toBeTruthy();
      expect(requeuedMsg!.retry_count).toBe(1);

      await synap.queue.ack(testQueue, requeuedMsg!.id);
    });
  });

  describe('Queue management', () => {
    it('should list queues', async () => {
      const queues = await synap.queue.listQueues();
      expect(Array.isArray(queues)).toBe(true);
      expect(queues).toContain(testQueue);
    });

    it('should get queue stats', async () => {
      const stats = await synap.queue.stats(testQueue);
      
      expect(stats).toHaveProperty('depth');
      expect(stats).toHaveProperty('published');
      expect(stats).toHaveProperty('consumed');
      expect(typeof stats.depth).toBe('number');
    });

    it('should purge queue', async () => {
      const purgeQueue = 'test_purge_queue';
      await synap.queue.createQueue(purgeQueue);

      // Add messages
      await synap.queue.publishString(purgeQueue, 'msg1');
      await synap.queue.publishString(purgeQueue, 'msg2');
      await synap.queue.publishString(purgeQueue, 'msg3');

      const purged = await synap.queue.purge(purgeQueue);
      expect(purged).toBe(3);

      await synap.queue.deleteQueue(purgeQueue);
    });

    it('should delete queue', async () => {
      await synap.queue.createQueue('test_delete_queue');
      const deleted = await synap.queue.deleteQueue('test_delete_queue');
      expect(deleted).toBe(true);
    });
  });
});

