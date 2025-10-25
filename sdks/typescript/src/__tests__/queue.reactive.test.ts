/**
 * Queue Reactive Unit Tests (Mock)
 * Unit tests using mocked client - no server required
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { QueueManager } from '../queue';
import { createMockClient } from './__mocks__/client.mock';
import { firstValueFrom, take, toArray } from 'rxjs';
import type { SynapClient } from '../client';

describe('QueueManager - Reactive (Unit Tests)', () => {
  let mockClient: SynapClient;
  let queue: QueueManager;

  beforeEach(() => {
    mockClient = createMockClient();
    queue = new QueueManager(mockClient);
  });

  describe('observeMessages()', () => {
    it('should create observable from queue messages', async () => {
      // Mock consume to return messages
      vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({
        message: {
          id: 'msg-1',
          payload: Array.from(new TextEncoder().encode(JSON.stringify({ task: 'task1' }))),
          priority: 5,
          retry_count: 0,
          max_retries: 3,
        }
      }).mockResolvedValueOnce({
        message: null // Empty queue after first message
      });

      const messages = await firstValueFrom(
        queue.observeMessages<{ task: string }>({
          queueName: 'test',
          consumerId: 'worker-1',
          pollingInterval: 10,
        }).pipe(
          take(1),
          toArray()
        )
      );

      expect(messages).toHaveLength(1);
      expect(messages[0].data.task).toBe('task1');
      expect(messages[0].message.id).toBe('msg-1');
      expect(typeof messages[0].ack).toBe('function');
      expect(typeof messages[0].nack).toBe('function');

      queue.stopConsumer('test', 'worker-1');
    });

    it('should handle empty queue', async () => {
      // Mock empty queue
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        message: null
      });

      let emittedCount = 0;
      const subscription = queue.observeMessages({
        queueName: 'empty',
        consumerId: 'worker',
        pollingInterval: 10,
      }).subscribe({
        next: () => emittedCount++
      });

      await new Promise(resolve => setTimeout(resolve, 50));

      expect(emittedCount).toBe(0);
      
      subscription.unsubscribe();
      queue.stopConsumer('empty', 'worker');
    });

    it('should support ACK operation', async () => {
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({
          message: {
            id: 'msg-ack',
            payload: Array.from(new TextEncoder().encode(JSON.stringify({ data: 'test' }))),
            priority: 5,
            retry_count: 0,
            max_retries: 3,
          }
        })
        .mockResolvedValueOnce({ success: true }); // ACK response

      const msg = await firstValueFrom(
        queue.observeMessages({
          queueName: 'test',
          consumerId: 'ack-test',
        }).pipe(take(1))
      );

      await msg.ack();

      // Verify ACK was called
      expect(mockClient.sendCommand).toHaveBeenCalledWith('queue.ack', {
        queue: 'test',
        message_id: 'msg-ack',
      });

      queue.stopConsumer('test', 'ack-test');
    });

    it('should support NACK operation', async () => {
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({
          message: {
            id: 'msg-nack',
            payload: Array.from(new TextEncoder().encode(JSON.stringify({ data: 'test' }))),
            priority: 5,
            retry_count: 0,
            max_retries: 3,
          }
        })
        .mockResolvedValueOnce({ success: true }); // NACK response

      const msg = await firstValueFrom(
        queue.observeMessages({
          queueName: 'test',
          consumerId: 'nack-test',
        }).pipe(take(1))
      );

      await msg.nack(true);

      // Verify NACK was called
      expect(mockClient.sendCommand).toHaveBeenCalledWith('queue.nack', {
        queue: 'test',
        message_id: 'msg-nack',
        requeue: true,
      });

      queue.stopConsumer('test', 'nack-test');
    });
  });

  describe('processMessages()', () => {
    it('should auto-ACK on successful processing', async () => {
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({
          message: {
            id: 'msg-process',
            payload: Array.from(new TextEncoder().encode(JSON.stringify({ value: 42 }))),
            priority: 5,
            retry_count: 0,
            max_retries: 3,
          }
        })
        .mockResolvedValueOnce({ success: true }) // ACK
        .mockResolvedValueOnce({ message: null }); // Empty after

      let processedValue = 0;

      const result = await firstValueFrom(
        queue.processMessages<{ value: number }>(
          {
            queueName: 'test',
            consumerId: 'processor',
            pollingInterval: 10,
          },
          async (data) => {
            processedValue = data.value;
          }
        ).pipe(take(1))
      );

      expect(result.success).toBe(true);
      expect(processedValue).toBe(42);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('queue.ack', expect.any(Object));

      queue.stopConsumer('test', 'processor');
    });

    it('should auto-NACK on processing error', async () => {
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({
          message: {
            id: 'msg-error',
            payload: Array.from(new TextEncoder().encode(JSON.stringify({ fail: true }))),
            priority: 5,
            retry_count: 0,
            max_retries: 3,
          }
        })
        .mockResolvedValueOnce({ success: true }) // NACK
        .mockResolvedValueOnce({ message: null }); // Empty

      const result = await firstValueFrom(
        queue.processMessages(
          {
            queueName: 'test',
            consumerId: 'error-processor',
            pollingInterval: 10,
          },
          async (data: any) => {
            if (data.fail) {
              throw new Error('Processing failed');
            }
          }
        ).pipe(take(1))
      );

      expect(result.success).toBe(false);
      expect(result.error).toBeTruthy();
      expect(mockClient.sendCommand).toHaveBeenCalledWith('queue.nack', expect.any(Object));

      queue.stopConsumer('test', 'error-processor');
    });
  });

  describe('observeStats()', () => {
    it('should emit queue stats', async () => {
      const mockStats = {
        depth: 5,
        consumers: 2,
        published: 100,
        consumed: 95,
        acked: 90,
        nacked: 5,
        dead_lettered: 0,
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockStats);

      const stats = await firstValueFrom(
        queue.observeStats('test', 10).pipe(take(2), toArray())
      );

      expect(stats).toHaveLength(2);
      expect(stats[0]).toEqual(mockStats);
      expect(stats[1]).toEqual(mockStats);
    });
  });

  describe('Lifecycle', () => {
    it('should stop consumer', () => {
      const subscription = queue.observeMessages({
        queueName: 'test',
        consumerId: 'stop-test',
      }).subscribe();

      // Should not throw
      expect(() => queue.stopConsumer('test', 'stop-test')).not.toThrow();
      
      subscription.unsubscribe();
    });

    it('should stop all consumers', () => {
      const sub1 = queue.observeMessages({ queueName: 'q1', consumerId: 'c1' }).subscribe();
      const sub2 = queue.observeMessages({ queueName: 'q2', consumerId: 'c2' }).subscribe();

      expect(() => queue.stopAllConsumers()).not.toThrow();

      sub1.unsubscribe();
      sub2.unsubscribe();
    });
  });
});
