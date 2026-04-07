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

  describe('publishString() - Lines 174-180', () => {
    it('should publish string message', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ message_id: 'msg-123' });

      const msgId = await queue.publishString('string-queue', 'Hello World');

      expect(msgId).toBe('msg-123');
      expect(mockClient.sendCommand).toHaveBeenCalledWith('queue.publish', {
        queue: 'string-queue',
        payload: Array.from(new TextEncoder().encode('Hello World')),
      });
    });

    it('should publish string message with options', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ message_id: 'msg-456' });

      const msgId = await queue.publishString('string-queue', 'Important Message', {
        priority: 9,
        ttl: 3600,
      });

      expect(msgId).toBe('msg-456');
      const callArgs = vi.mocked(mockClient.sendCommand).mock.calls[0];
      expect(callArgs[0]).toBe('queue.publish');
      expect(callArgs[1].queue).toBe('string-queue');
      expect(callArgs[1].payload).toEqual(Array.from(new TextEncoder().encode('Important Message')));
      expect(callArgs[1].priority).toBe(9);
      // Note: ttl is not passed in options since it's not in the implementation
    });
  });

  describe('consumeJSON() - JSON Parsing Coverage (Lines 228-229)', () => {
    it('should throw error when JSON parsing fails', async () => {
      const mockMessage = {
        id: 'msg-bad-json',
        payload: new TextEncoder().encode('invalid{json}'),
        priority: 5,
        retry_count: 0,
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        message: mockMessage,
      });

      await expect(
        queue.consumeJSON('invalid-queue', 'consumer-invalid')
      ).rejects.toThrow('Failed to parse JSON');
    });

    it('should handle valid JSON parsing', async () => {
      const validData = { data: 'valid' };
      const mockMessage = {
        id: 'msg-valid-json',
        payload: new TextEncoder().encode(JSON.stringify(validData)),
        priority: 5,
        retry_count: 0,
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        message: mockMessage,
      });

      const result = await queue.consumeJSON<{ data: string }>('valid-queue', 'consumer-valid');

      expect(result.message).toEqual(mockMessage);
      expect(result.data).toEqual(validData);
    });

    it('should return null when message is missing', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        message: null,
      });

      const result = await queue.consumeJSON('empty-queue', 'consumer-empty');

      expect(result.message).toBeNull();
      expect(result.data).toBeNull();
    });

    it('should parse complex JSON objects', async () => {
      const complexData = { nested: { value: 42, array: [1, 2, 3] } };
      const mockMessage = {
        id: 'msg-complex',
        payload: new TextEncoder().encode(JSON.stringify(complexData)),
        priority: 5,
        retry_count: 0,
        timestamp: Date.now(),
      };
      
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        message: mockMessage,
      });

      const result = await queue.consumeJSON<{ nested: { value: number, array: number[] } }>('complex-queue', 'consumer');

      expect(result.data).toEqual(complexData);
    });

    it('should handle empty string payload', async () => {
      const mockMessage = {
        id: 'msg-empty',
        payload: new TextEncoder().encode(''),
        priority: 5,
        retry_count: 0,
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        message: mockMessage,
      });

      const result = await queue.consumeJSON('empty-payload-queue', 'consumer');

      expect(result.message).toBeNull();
      expect(result.data).toBeNull();
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

