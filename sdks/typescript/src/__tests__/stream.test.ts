/**
 * Stream Unit Tests (Mock)
 * Unit tests using mocked client - no server required
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { StreamManager } from '../stream';
import { createMockClient } from './__mocks__/client.mock';
import { firstValueFrom, take, toArray } from 'rxjs';
import type { SynapClient } from '../client';

describe('StreamManager (Unit Tests)', () => {
  let mockClient: SynapClient;
  let stream: StreamManager;

  beforeEach(() => {
    mockClient = createMockClient();
    stream = new StreamManager(mockClient);
  });

  describe('Room Management', () => {
    it('should create room', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ success: true });

      const result = await stream.createRoom('test-room');
      
      expect(result).toBe(true);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('stream.create', {
        room: 'test-room',
      });
    });

    it('should list rooms', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ 
        rooms: ['room1', 'room2', 'room3'] 
      });

      const rooms = await stream.listRooms();
      
      expect(rooms).toEqual(['room1', 'room2', 'room3']);
    });

    it('should get stream stats', async () => {
      const mockStats = {
        max_offset: 42,
        subscribers: 3,
        total_events: 100,
        total_consumed: 95,
        room: 'test-room',
        created_at: Date.now(),
        last_activity: Date.now(),
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockStats);

      const stats = await stream.stats('test-room');
      
      expect(stats).toEqual(mockStats);
      expect(stats.max_offset).toBe(42);
    });

    it('should delete room', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ deleted: 'test-room' });

      const result = await stream.deleteRoom('test-room');
      
      expect(result).toBe(true);
    });
  });

  describe('Publish/Consume', () => {
    it('should publish event and return offset', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ offset: 10 });

      const offset = await stream.publish('room', 'event.test', { data: 'value' });
      
      expect(offset).toBe(10);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('stream.publish', {
        room: 'room',
        event: 'event.test',
        data: { data: 'value' },
      });
    });

    it('should consume events', async () => {
      const mockEvents = [
        {
          offset: 0,
          event: 'test.event1',
          data: Array.from(new TextEncoder().encode(JSON.stringify({ value: 1 }))),
          timestamp: Date.now(),
        },
        {
          offset: 1,
          event: 'test.event2',
          data: Array.from(new TextEncoder().encode(JSON.stringify({ value: 2 }))),
          timestamp: Date.now(),
        }
      ];

      vi.mocked(mockClient.sendCommand).mockResolvedValue({ events: mockEvents });

      const events = await stream.consume('room', 'subscriber', 0);
      
      expect(events).toHaveLength(2);
      expect(events[0].data.value).toBe(1);
      expect(events[1].data.value).toBe(2);
    });

    it('should parse event data correctly', async () => {
      const mockEvent = {
        offset: 0,
        event: 'user.created',
        data: Array.from(new TextEncoder().encode(JSON.stringify({ 
          userId: '123', 
          name: 'Alice' 
        }))),
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue({ events: [mockEvent] });

      const events = await stream.consume('room', 'sub', 0);
      
      expect(events[0].data.userId).toBe('123');
      expect(events[0].data.name).toBe('Alice');
    });
  });

  describe('observeEvents()', () => {
    it('should create observable stream', async () => {
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({
          events: [{
            offset: 0,
            event: 'test.event',
            data: Array.from(new TextEncoder().encode(JSON.stringify({ value: 1 }))),
          }]
        })
        .mockResolvedValue({ events: [] }); // Empty after first

      const events = await firstValueFrom(
        stream.observeEvents<{ value: number }>({
          roomName: 'room',
          subscriberId: 'sub',
          fromOffset: 0,
          pollingInterval: 10,
        }).pipe(take(1), toArray())
      );

      expect(events).toHaveLength(1);
      expect(events[0].data.value).toBe(1);

      stream.stopConsumer('room', 'sub');
    });

    it('should track offset correctly', async () => {
      let callCount = 0;
      vi.mocked(mockClient.sendCommand).mockImplementation(async (cmd, payload: any) => {
        if (cmd === 'stream.consume') {
          callCount++;
          if (callCount === 1) {
            return {
              events: [{
                offset: 0,
                event: 'event1',
                data: Array.from(new TextEncoder().encode(JSON.stringify({ n: 1 }))),
              }]
            };
          }
          if (callCount === 2) {
            // Should request from offset 1
            expect(payload.from_offset).toBe(1);
            return { events: [] };
          }
        }
        return { events: [] };
      });

      const subscription = stream.observeEvents({
        roomName: 'room',
        subscriberId: 'offset-test',
        fromOffset: 0,
        pollingInterval: 10,
      }).subscribe();

      await new Promise(resolve => setTimeout(resolve, 50));

      subscription.unsubscribe();
      stream.stopConsumer('room', 'offset-test');
    });
  });

  describe('observeEvent()', () => {
    it('should filter by event name', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        events: [
          {
            offset: 0,
            event: 'user.login',
            data: Array.from(new TextEncoder().encode(JSON.stringify({ user: 'alice' }))),
          },
          {
            offset: 1,
            event: 'user.logout',
            data: Array.from(new TextEncoder().encode(JSON.stringify({ user: 'bob' }))),
          },
        ]
      });

      const loginEvents = await firstValueFrom(
        stream.observeEvent<{ user: string }>({
          roomName: 'room',
          subscriberId: 'filter',
          fromOffset: 0,
          eventName: 'user.login',
          pollingInterval: 10,
        }).pipe(take(1), toArray())
      );

      expect(loginEvents).toHaveLength(1);
      expect(loginEvents[0].event).toBe('user.login');
      expect(loginEvents[0].data.user).toBe('alice');

      stream.stopConsumer('room', 'filter');
    });
  });

  describe('observeStats()', () => {
    it('should emit stats at intervals', async () => {
      const mockStats = {
        max_offset: 10,
        subscribers: 2,
        total_events: 50,
        total_consumed: 45,
        room: 'room',
        created_at: Date.now(),
        last_activity: Date.now(),
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockStats);

      const stats = await firstValueFrom(
        stream.observeStats('room', 10).pipe(take(2), toArray())
      );

      expect(stats).toHaveLength(2);
      expect(stats[0].max_offset).toBe(10);
    });
  });

  describe('Lifecycle', () => {
    it('should stop consumer', () => {
      const sub = stream.observeEvents({ roomName: 'r', subscriberId: 's' }).subscribe();
      
      expect(() => stream.stopConsumer('r', 's')).not.toThrow();
      
      sub.unsubscribe();
    });

    it('should stop all consumers', () => {
      const sub1 = stream.observeEvents({ roomName: 'r1', subscriberId: 's1' }).subscribe();
      const sub2 = stream.observeEvents({ roomName: 'r2', subscriberId: 's2' }).subscribe();

      expect(() => stream.stopAllConsumers()).not.toThrow();

      sub1.unsubscribe();
      sub2.unsubscribe();
    });
  });
});
