/**
 * Stream Unit Tests - Additional Coverage
 * Unit tests to cover uncovered lines in stream.ts
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { StreamManager } from '../stream';
import { createMockClient } from './__mocks__/client.mock';
import type { SynapClient } from '../client';
import { take, toArray } from 'rxjs/operators';
import { firstValueFrom } from 'rxjs';

describe('StreamManager (Unit Tests - Additional Coverage)', () => {
  let mockClient: SynapClient;
  let stream: StreamManager;

  beforeEach(() => {
    mockClient = createMockClient();
    stream = new StreamManager(mockClient);
  });

  describe('observeEvent() - Lines 218-226', () => {
    it('should filter events by name', async () => {
      const mockEvents = [
        {
          offset: 0,
          event: 'user.login',
          data: Array.from(new TextEncoder().encode(JSON.stringify({ user: 'alice' }))),
          timestamp: Date.now(),
        },
        {
          offset: 1,
          event: 'user.logout',
          data: Array.from(new TextEncoder().encode(JSON.stringify({ user: 'bob' }))),
          timestamp: Date.now(),
        },
        {
          offset: 2,
          event: 'user.login',
          data: Array.from(new TextEncoder().encode(JSON.stringify({ user: 'charlie' }))),
          timestamp: Date.now(),
        },
      ];

      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ events: mockEvents })
        .mockResolvedValue({ events: [] });

      const loginEvents = await firstValueFrom(
        stream.observeEvent<{ user: string }>({
          roomName: 'users',
          subscriberId: 'filter-test',
          fromOffset: 0,
          eventName: 'user.login',
          pollingInterval: 10,
        }).pipe(take(2), toArray())
      );

      expect(loginEvents).toHaveLength(2);
      expect(loginEvents[0].event).toBe('user.login');
      expect(loginEvents[0].data.user).toBe('alice');
      expect(loginEvents[1].event).toBe('user.login');
      expect(loginEvents[1].data.user).toBe('charlie');

      stream.stopConsumer('users', 'filter-test');
    });

    it('should filter out non-matching events', async () => {
      const mockEvents = [
        {
          offset: 0,
          event: 'order.created',
          data: Array.from(new TextEncoder().encode(JSON.stringify({ id: 1 }))),
          timestamp: Date.now(),
        },
        {
          offset: 1,
          event: 'order.updated',
          data: Array.from(new TextEncoder().encode(JSON.stringify({ id: 2 }))),
          timestamp: Date.now(),
        },
        {
          offset: 2,
          event: 'order.created',
          data: Array.from(new TextEncoder().encode(JSON.stringify({ id: 3 }))),
          timestamp: Date.now(),
        },
      ];

      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ events: mockEvents })
        .mockResolvedValue({ events: [] });

      const createdEvents = await firstValueFrom(
        stream.observeEvent({
          roomName: 'orders',
          subscriberId: 'created-only',
          fromOffset: 0,
          eventName: 'order.created',
          pollingInterval: 10,
        }).pipe(take(2), toArray())
      );

      expect(createdEvents).toHaveLength(2);
      expect(createdEvents.every(e => e.event === 'order.created')).toBe(true);

      stream.stopConsumer('orders', 'created-only');
    });
  });

  describe('observeStats() - Lines 269-282', () => {
    it('should emit stats at regular intervals', async () => {
      const mockStats = {
        max_offset: 100,
        subscribers: 5,
        total_events: 1000,
        total_consumed: 950,
        room: 'test-room',
        created_at: Date.now(),
        last_activity: Date.now(),
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockStats);

      const stats = await firstValueFrom(
        stream.observeStats('test-room', 10).pipe(take(3), toArray())
      );

      expect(stats).toHaveLength(3);
      expect(stats[0].max_offset).toBe(100);
      expect(stats[0].subscribers).toBe(5);
      expect(stats[1].room).toBe('test-room');
    });

    it('should retry on errors', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      const mockStats = {
        max_offset: 50,
        subscribers: 2,
        total_events: 500,
        total_consumed: 450,
        room: 'retry-room',
        created_at: Date.now(),
        last_activity: Date.now(),
      };

      vi.mocked(mockClient.sendCommand)
        .mockRejectedValueOnce(new Error('Network error'))
        .mockRejectedValueOnce(new Error('Network error'))
        .mockResolvedValue(mockStats);

      const stats = await firstValueFrom(
        stream.observeStats('retry-room', 10).pipe(take(1))
      );

      expect(stats.max_offset).toBe(50);

      consoleSpy.mockRestore();
    });

    it('should handle continuous errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      vi.mocked(mockClient.sendCommand).mockRejectedValue(new Error('Persistent error'));

      const observable = stream.observeStats('error-room', 10);
      const receivedValues: any[] = [];
      
      const subscription = observable.subscribe({
        next: (val) => receivedValues.push(val),
        error: (err) => {
          // Should not error out (catchError handles it)
          expect(err).toBeUndefined();
        },
      });

      // Wait for retry attempts
      await new Promise(resolve => setTimeout(resolve, 100));

      subscription.unsubscribe();
      
      // Should have tried and caught errors
      expect(receivedValues).toHaveLength(0); // No successful stats
      consoleSpy.mockRestore();
    });

    it('should handle fast polling intervals', async () => {
      const mockStats = {
        max_offset: 10,
        subscribers: 1,
        total_events: 100,
        total_consumed: 100,
        room: 'fast-room',
        created_at: Date.now(),
        last_activity: Date.now(),
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockStats);

      const stats = await firstValueFrom(
        stream.observeStats('fast-room', 5).pipe(take(5), toArray())
      );

      expect(stats).toHaveLength(5);
      expect(stats.every(s => s.room === 'fast-room')).toBe(true);
    });
  });

  describe('publishEvent() - Lines 287-294', () => {
    it('should publish typed event', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ offset: 42 });

      interface UserEvent {
        userId: string;
        action: string;
      }

      const offset = await stream.publishEvent<UserEvent>(
        'events',
        'user.action',
        { userId: '123', action: 'login' }
      );

      expect(offset).toBe(42);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('stream.publish', {
        room: 'events',
        event: 'user.action',
        data: { userId: '123', action: 'login' },
      });
    });

    it('should publish with options', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ offset: 100 });

      const offset = await stream.publishEvent(
        'notifications',
        'notification.sent',
        { message: 'Hello' },
        { metadata: { priority: 'high' } }
      );

      expect(offset).toBe(100);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('stream.publish', {
        room: 'notifications',
        event: 'notification.sent',
        data: { message: 'Hello' },
        metadata: { priority: 'high' },
      });
    });

    it('should handle complex data types', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ offset: 200 });

      interface ComplexEvent {
        nested: {
          array: number[];
          object: {
            key: string;
          };
        };
      }

      const complexData: ComplexEvent = {
        nested: {
          array: [1, 2, 3],
          object: { key: 'value' },
        },
      };

      const offset = await stream.publishEvent('complex', 'complex.event', complexData);

      expect(offset).toBe(200);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('stream.publish', {
        room: 'complex',
        event: 'complex.event',
        data: complexData,
      });
    });
  });

  describe('Lifecycle Management - Lines 234-254', () => {
    it('should stop specific consumer', () => {
      const observable = stream.observeEvents({
        roomName: 'lifecycle-room',
        subscriberId: 'lifecycle-sub',
      });

      const subscription = observable.subscribe();

      expect(() => stream.stopConsumer('lifecycle-room', 'lifecycle-sub')).not.toThrow();

      subscription.unsubscribe();
    });

    it('should handle stopping non-existent consumer', () => {
      expect(() => stream.stopConsumer('nonexistent', 'sub')).not.toThrow();
    });

    it('should stop all consumers', () => {
      const sub1 = stream.observeEvents({
        roomName: 'room1',
        subscriberId: 'sub1',
      }).subscribe();

      const sub2 = stream.observeEvents({
        roomName: 'room2',
        subscriberId: 'sub2',
      }).subscribe();

      expect(() => stream.stopAllConsumers()).not.toThrow();

      sub1.unsubscribe();
      sub2.unsubscribe();
    });

    it('should handle multiple stop calls', () => {
      const observable = stream.observeEvents({
        roomName: 'multi-stop',
        subscriberId: 'multi-sub',
      });

      const subscription = observable.subscribe();

      stream.stopConsumer('multi-stop', 'multi-sub');
      stream.stopConsumer('multi-stop', 'multi-sub'); // Should not throw

      subscription.unsubscribe();
    });
  });

  describe('parseEventData() - Error Handling (Lines 107-111)', () => {
    it('should handle array byte data parsing errors', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      
      // Create invalid byte array that cannot be decoded properly
      const invalidByteArray = [255, 254, 253, 252];
      
      const mockEvent = {
        offset: 0,
        event: 'test.event',
        data: invalidByteArray, // Invalid UTF-8 sequence
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ events: [mockEvent] })
        .mockResolvedValue({ events: [] });

      const events = await firstValueFrom(
        stream.observeEvents({
          roomName: 'parse-error-room',
          subscriberId: 'parse-error-sub',
          fromOffset: 0,
          pollingInterval: 10,
        }).pipe(take(1), toArray())
      );

      // Should still emit the event with raw data
      expect(events).toHaveLength(1);
      expect(events[0].data).toEqual(invalidByteArray);
      
      stream.stopConsumer('parse-error-room', 'parse-error-sub');
      consoleSpy.mockRestore();
    });

    it('should handle valid byte array parsing', async () => {
      const validData = { message: 'Hello' };
      const byteArray = Array.from(new TextEncoder().encode(JSON.stringify(validData)));
      
      const mockEvent = {
        offset: 0,
        event: 'test.event',
        data: byteArray,
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ events: [mockEvent] })
        .mockResolvedValue({ events: [] });

      const events = await firstValueFrom(
        stream.observeEvents<{ message: string }>({
          roomName: 'valid-parse-room',
          subscriberId: 'valid-parse-sub',
          fromOffset: 0,
          pollingInterval: 10,
        }).pipe(take(1), toArray())
      );

      expect(events).toHaveLength(1);
      expect(events[0].data.message).toBe('Hello');
      
      stream.stopConsumer('valid-parse-room', 'valid-parse-sub');
    });

    it('should handle non-array data directly', async () => {
      const directData = { key: 'value' };
      
      const mockEvent = {
        offset: 0,
        event: 'test.event',
        data: directData, // Not an array
        timestamp: Date.now(),
      };

      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ events: [mockEvent] })
        .mockResolvedValue({ events: [] });

      const events = await firstValueFrom(
        stream.observeEvents<{ key: string }>({
          roomName: 'direct-data-room',
          subscriberId: 'direct-data-sub',
          fromOffset: 0,
          pollingInterval: 10,
        }).pipe(take(1), toArray())
      );

      expect(events).toHaveLength(1);
      expect(events[0].data.key).toBe('value');
      
      stream.stopConsumer('direct-data-room', 'direct-data-sub');
    });
  });

  describe('Error Handling in observeEvents', () => {
    it('should handle consume errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      vi.mocked(mockClient.sendCommand).mockRejectedValue(new Error('Consume error'));

      const observable = stream.observeEvents({
        roomName: 'error-room',
        subscriberId: 'error-sub',
        pollingInterval: 10,
      });

      const subscription = observable.subscribe({
        error: (err) => {
          // Should not error out
          expect(err).toBeUndefined();
        },
      });

      await new Promise(resolve => setTimeout(resolve, 50));

      subscription.unsubscribe();
      stream.stopConsumer('error-room', 'error-sub');
      
      expect(consoleSpy).toHaveBeenCalled();
      consoleSpy.mockRestore();
    });
  });
});

