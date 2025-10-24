/**
 * Queue Unit Tests - Additional Coverage
 * Unit tests to cover uncovered lines in queue.ts
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { QueueManager } from '../queue';
import { createMockClient } from './__mocks__/client.mock';
import type { SynapClient } from '../client';
import { take, toArray } from 'rxjs/operators';
import { firstValueFrom } from 'rxjs';

describe('QueueManager (Unit Tests - Additional Coverage)', () => {
  let mockClient: SynapClient;
  let queue: QueueManager;

  beforeEach(() => {
    mockClient = createMockClient();
    queue = new QueueManager(mockClient);
  });

  describe('observeMessages() - Lines 253-299', () => {
    it('should create observable with all options', async () => {
      const mockMessage = {
        id: 'msg-1',
        payload: Array.from(new TextEncoder().encode(JSON.stringify({ task: 'test' }))),
        priority: 5,
        retry_count: 0,
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ message: mockMessage, data: { task: 'test' } })
        .mockResolvedValue({ message: null, data: null });

      const messages = await firstValueFrom(
        queue.observeMessages<{ task: string }>({
          queueName: 'test-queue',
          consumerId: 'consumer-1',
          pollingInterval: 10,
          concurrency: 2,
          requeueOnNack: false,
        }).pipe(take(1), toArray())
      );

      expect(messages).toHaveLength(1);
      expect(messages[0].data.task).toBe('test');
      expect(typeof messages[0].ack).toBe('function');
      expect(typeof messages[0].nack).toBe('function');

      queue.stopConsumer('test-queue', 'consumer-1');
    });

    it('should handle null messages', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        message: null,
        data: null,
      });

      const observable = queue.observeMessages({
        queueName: 'empty-queue',
        consumerId: 'consumer-2',
        pollingInterval: 10,
      });

      const subscription = observable.subscribe();
      
      await new Promise(resolve => setTimeout(resolve, 50));

      subscription.unsubscribe();
      queue.stopConsumer('empty-queue', 'consumer-2');
    });

    it('should handle errors in consume', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      
      vi.mocked(mockClient.sendCommand).mockRejectedValue(new Error('Network error'));

      const observable = queue.observeMessages({
        queueName: 'error-queue',
        consumerId: 'consumer-3',
        pollingInterval: 10,
      });

      const subscription = observable.subscribe();

      await new Promise(resolve => setTimeout(resolve, 50));

      expect(consoleSpy).toHaveBeenCalled();

      subscription.unsubscribe();
      queue.stopConsumer('error-queue', 'consumer-3');
      consoleSpy.mockRestore();
    });

    it('should call ack correctly', async () => {
      const mockMessage = {
        id: 'msg-ack-test',
        payload: Array.from(new TextEncoder().encode(JSON.stringify({ data: 'test' }))),
        priority: 5,
        retry_count: 0,
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ message: mockMessage, data: { data: 'test' } })
        .mockResolvedValue({ message: null, data: null });

      const msg = await firstValueFrom(
        queue.observeMessages({
          queueName: 'ack-queue',
          consumerId: 'ack-consumer',
          pollingInterval: 10,
        }).pipe(take(1))
      );

      // Mock for ack call
      vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ success: true });

      await msg.ack();

      // Should have been called for consume + ack
      expect(mockClient.sendCommand).toHaveBeenCalledWith('queue.ack', {
        queue: 'ack-queue',
        message_id: 'msg-ack-test',
      });

      queue.stopConsumer('ack-queue', 'ack-consumer');
    });

    it('should call nack correctly with requeue', async () => {
      const mockMessage = {
        id: 'msg-nack-test',
        payload: Array.from(new TextEncoder().encode(JSON.stringify({ data: 'test' }))),
        priority: 5,
        retry_count: 0,
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ message: mockMessage, data: { data: 'test' } })
        .mockResolvedValue({ message: null, data: null });

      const msg = await firstValueFrom(
        queue.observeMessages({
          queueName: 'nack-queue',
          consumerId: 'nack-consumer',
          pollingInterval: 10,
          requeueOnNack: true,
        }).pipe(take(1))
      );

      // Mock for nack call
      vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ success: true });

      await msg.nack(true);

      expect(mockClient.sendCommand).toHaveBeenCalledWith('queue.nack', {
        queue: 'nack-queue',
        message_id: 'msg-nack-test',
        requeue: true,
      });

      queue.stopConsumer('nack-queue', 'nack-consumer');
    });
  });

  describe('observeMessagesAuto() - Lines 318-326', () => {
    it('should create auto-ack observable', async () => {
      const mockMessage = {
        id: 'auto-msg',
        payload: Array.from(new TextEncoder().encode(JSON.stringify({ value: 42 }))),
        priority: 5,
        retry_count: 0,
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ message: mockMessage, data: { value: 42 } })
        .mockResolvedValue({ message: null, data: null });

      const messages = await firstValueFrom(
        queue.observeMessagesAuto<{ value: number }>({
          queueName: 'auto-queue',
          consumerId: 'auto-consumer',
          pollingInterval: 10,
        }).pipe(take(1), toArray())
      );

      expect(messages).toHaveLength(1);
      expect(messages[0].data.value).toBe(42);

      queue.stopConsumer('auto-queue', 'auto-consumer');
    });
  });

  describe('observeStats() - Lines 411-424', () => {
    it('should emit stats at intervals', async () => {
      const mockStats = {
        depth: 10,
        published: 50,
        consumed: 40,
        pending: 10,
        failed: 0,
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockStats);

      const stats = await firstValueFrom(
        queue.observeStats('stats-queue', 10).pipe(take(2), toArray())
      );

      expect(stats).toHaveLength(2);
      expect(stats[0].depth).toBe(10);
      expect(stats[1].published).toBe(50);
    });

    it('should retry on error', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      vi.mocked(mockClient.sendCommand)
        .mockRejectedValueOnce(new Error('Network error'))
        .mockRejectedValueOnce(new Error('Network error'))
        .mockResolvedValue({
          depth: 5,
          published: 10,
          consumed: 5,
          pending: 5,
          failed: 0,
        });

      const stats = await firstValueFrom(
        queue.observeStats('retry-queue', 10).pipe(take(1))
      );

      expect(stats.depth).toBe(5);

      consoleSpy.mockRestore();
    });

    it('should handle continuous errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      vi.mocked(mockClient.sendCommand).mockRejectedValue(new Error('Persistent error'));

      const observable = queue.observeStats('error-queue', 10);
      const subscription = observable.subscribe({
        next: () => {
          // Should not receive any values
        },
        error: (err) => {
          // Should not error out
          expect(err).toBeUndefined();
        },
      });

      await new Promise(resolve => setTimeout(resolve, 50));

      subscription.unsubscribe();
      consoleSpy.mockRestore();
    });
  });

  describe('Lifecycle Management', () => {
    it('should stop consumer by key', () => {
      const observable = queue.observeMessages({
        queueName: 'lifecycle-queue',
        consumerId: 'lifecycle-consumer',
      });

      const subscription = observable.subscribe();

      expect(() => queue.stopConsumer('lifecycle-queue', 'lifecycle-consumer')).not.toThrow();

      subscription.unsubscribe();
    });

    it('should handle stopping non-existent consumer', () => {
      expect(() => queue.stopConsumer('nonexistent', 'consumer')).not.toThrow();
    });

    it('should stop all consumers', () => {
      const sub1 = queue.observeMessages({
        queueName: 'q1',
        consumerId: 'c1',
      }).subscribe();

      const sub2 = queue.observeMessages({
        queueName: 'q2',
        consumerId: 'c2',
      }).subscribe();

      expect(() => queue.stopAllConsumers()).not.toThrow();

      sub1.unsubscribe();
      sub2.unsubscribe();
    });
  });
});

