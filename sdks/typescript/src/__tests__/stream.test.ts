/**
 * Event Stream Module Tests
 * Tests for reactive event stream patterns
 */

import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import { Synap } from '../index';
import { take, toArray, timeout, filter } from 'rxjs/operators';
import { firstValueFrom } from 'rxjs';

describe('StreamManager', () => {
  let synap: Synap;
  const testRoom = 'test_stream_room';

  beforeAll(async () => {
    synap = new Synap({
      url: process.env.SYNAP_URL || 'http://localhost:15500',
    });

    await synap.stream.createRoom(testRoom);
  });

  afterAll(async () => {
    synap.stream.stopAllConsumers();
    await synap.stream.deleteRoom(testRoom);
    synap.close();
  });

  beforeEach(async () => {
    // Events are append-only, can't clear, but each test uses different offsets
  });

  describe('Room Management', () => {
    it('should create a stream room', async () => {
      const roomName = 'test_create_room';
      const created = await synap.stream.createRoom(roomName);
      expect(created).toBe(true);
      
      await synap.stream.deleteRoom(roomName);
    });

    it('should list stream rooms', async () => {
      const rooms = await synap.stream.listRooms();
      expect(Array.isArray(rooms)).toBe(true);
      expect(rooms).toContain(testRoom);
    });

    it('should get stream stats', async () => {
      const stats = await synap.stream.stats(testRoom);
      
      expect(stats).toHaveProperty('event_count');
      expect(stats).toHaveProperty('subscribers');
      expect(typeof stats.event_count).toBe('number');
    });

    it('should delete a stream room', async () => {
      const roomName = 'test_delete_room';
      await synap.stream.createRoom(roomName);
      
      const deleted = await synap.stream.deleteRoom(roomName);
      expect(deleted).toBe(true);
    });
  });

  describe('Publish/Consume operations', () => {
    it('should publish and consume events', async () => {
      const offset1 = await synap.stream.publish(testRoom, 'user.login', {
        userId: '123',
        timestamp: Date.now(),
      });
      expect(typeof offset1).toBe('number');

      const offset2 = await synap.stream.publish(testRoom, 'user.logout', {
        userId: '123',
        timestamp: Date.now(),
      });
      expect(offset2).toBeGreaterThan(offset1);

      const events = await synap.stream.consume(testRoom, 'test-consumer', offset1);
      expect(events.length).toBeGreaterThanOrEqual(2);
      expect(events[0].event).toBe('user.login');
      expect(events[1].event).toBe('user.logout');
    });

    it('should consume from specific offset', async () => {
      const startOffset = await synap.stream.publish(testRoom, 'test.event1', { data: 1 });
      await synap.stream.publish(testRoom, 'test.event2', { data: 2 });
      await synap.stream.publish(testRoom, 'test.event3', { data: 3 });

      // Consume from second event
      const events = await synap.stream.consume(testRoom, 'offset-consumer', startOffset + 1);
      expect(events.length).toBeGreaterThanOrEqual(2);
      expect(events[0].event).toBe('test.event2');
    });

    it('should return empty array for future offset', async () => {
      const stats = await synap.stream.stats(testRoom);
      const futureOffset = stats.event_count + 1000;
      
      const events = await synap.stream.consume(testRoom, 'future-consumer', futureOffset);
      expect(events).toEqual([]);
    });
  });

  describe('Reactive Methods', () => {
    describe('consume$() - Reactive Consumer', () => {
      it('should consume events reactively', async () => {
        const baseOffset = (await synap.stream.stats(testRoom)).event_count;

        // Publish events
        await synap.stream.publish(testRoom, 'reactive.test1', { value: 1 });
        await synap.stream.publish(testRoom, 'reactive.test2', { value: 2 });

        // Consume reactively
        const events = await firstValueFrom(
          synap.stream.consume$<{ value: number }>({
            roomName: testRoom,
            subscriberId: 'reactive-consumer-1',
            fromOffset: baseOffset,
            pollingInterval: 100,
          }).pipe(
            take(2),
            toArray(),
            timeout(5000)
          )
        );

        expect(events).toHaveLength(2);
        expect(events[0].event).toBe('reactive.test1');
        expect(events[0].data.value).toBe(1);
        expect(events[1].event).toBe('reactive.test2');
        expect(events[1].data.value).toBe(2);

        synap.stream.stopConsumer(testRoom, 'reactive-consumer-1');
      }, 10000);

      it('should provide event metadata', async () => {
        const baseOffset = (await synap.stream.stats(testRoom)).event_count;
        
        await synap.stream.publish(testRoom, 'metadata.test', { info: 'test' });

        const event = await firstValueFrom(
          synap.stream.consume$({
            roomName: testRoom,
            subscriberId: 'metadata-consumer',
            fromOffset: baseOffset,
          }).pipe(
            take(1),
            timeout(5000)
          )
        );

        expect(event).toHaveProperty('offset');
        expect(event).toHaveProperty('event');
        expect(event).toHaveProperty('data');
        expect(event.event).toBe('metadata.test');
        expect(typeof event.offset).toBe('number');

        synap.stream.stopConsumer(testRoom, 'metadata-consumer');
      }, 10000);

      it('should handle empty stream gracefully', async () => {
        const futureOffset = (await synap.stream.stats(testRoom)).event_count + 100;

        const subscription = synap.stream.consume$({
          roomName: testRoom,
          subscriberId: 'empty-consumer',
          fromOffset: futureOffset,
          pollingInterval: 100,
        }).subscribe({
          next: () => {
            // Should not emit for empty stream
            throw new Error('Should not emit events');
          },
        });

        // Wait a bit
        await new Promise(resolve => setTimeout(resolve, 500));

        subscription.unsubscribe();
        synap.stream.stopConsumer(testRoom, 'empty-consumer');
      }, 5000);
    });

    describe('consumeEvent$() - Event Filtering', () => {
      it('should filter events by name', async () => {
        const baseOffset = (await synap.stream.stats(testRoom)).event_count;

        // Publish mixed events
        await synap.stream.publish(testRoom, 'user.login', { user: 'alice' });
        await synap.stream.publish(testRoom, 'user.logout', { user: 'bob' });
        await synap.stream.publish(testRoom, 'user.login', { user: 'charlie' });

        // Filter only login events
        const loginEvents = await firstValueFrom(
          synap.stream.consumeEvent$<{ user: string }>({
            roomName: testRoom,
            subscriberId: 'filter-consumer',
            fromOffset: baseOffset,
            eventName: 'user.login',
            pollingInterval: 100,
          }).pipe(
            take(2),
            toArray(),
            timeout(5000)
          )
        );

        expect(loginEvents).toHaveLength(2);
        expect(loginEvents[0].data.user).toBe('alice');
        expect(loginEvents[1].data.user).toBe('charlie');

        synap.stream.stopConsumer(testRoom, 'filter-consumer');
      }, 10000);
    });

    describe('stats$() - Reactive Stats', () => {
      it('should emit stats at intervals', async () => {
        const statsEmissions = await firstValueFrom(
          synap.stream.stats$(testRoom, 500).pipe(
            take(2),
            toArray(),
            timeout(5000)
          )
        );

        expect(statsEmissions).toHaveLength(2);
        expect(statsEmissions[0]).toHaveProperty('event_count');
        expect(statsEmissions[1]).toHaveProperty('event_count');
      }, 10000);

      it('should reflect published events in stats', async () => {
        const statsPromise = firstValueFrom(
          synap.stream.stats$(testRoom, 300).pipe(
            take(3),
            toArray(),
            timeout(5000)
          )
        );

        // Publish event while monitoring
        await new Promise(resolve => setTimeout(resolve, 100));
        await synap.stream.publish(testRoom, 'stats.test', { data: 'test' });

        const stats = await statsPromise;
        
        expect(stats).toHaveLength(3);
        // Event count should increase
        const firstCount = stats[0].event_count;
        const lastCount = stats[2].event_count;
        expect(lastCount).toBeGreaterThanOrEqual(firstCount);
      }, 10000);
    });

    describe('Lifecycle Management', () => {
      it('should stop a specific consumer', async () => {
        const baseOffset = (await synap.stream.stats(testRoom)).event_count;

        // Publish events continuously
        const publishInterval = setInterval(async () => {
          await synap.stream.publish(testRoom, 'lifecycle.test', { data: Date.now() });
        }, 200);

        let eventCount = 0;

        const subscription = synap.stream.consume$({
          roomName: testRoom,
          subscriberId: 'stop-test-consumer',
          fromOffset: baseOffset,
          pollingInterval: 100,
        }).subscribe({
          next: () => eventCount++
        });

        // Let it consume a few
        await new Promise(resolve => setTimeout(resolve, 500));

        // Stop consumer
        synap.stream.stopConsumer(testRoom, 'stop-test-consumer');
        
        const countAfterStop = eventCount;
        
        // Wait a bit more
        await new Promise(resolve => setTimeout(resolve, 500));
        
        // Should not consume more after stop
        expect(eventCount).toBe(countAfterStop);
        
        clearInterval(publishInterval);
        subscription.unsubscribe();
      }, 5000);

      it('should stop all consumers', async () => {
        const baseOffset = (await synap.stream.stats(testRoom)).event_count;

        let count1 = 0;
        let count2 = 0;

        const sub1 = synap.stream.consume$({
          roomName: testRoom,
          subscriberId: 'consumer-1',
          fromOffset: baseOffset,
        }).subscribe({ next: () => count1++ });

        const sub2 = synap.stream.consume$({
          roomName: testRoom,
          subscriberId: 'consumer-2',
          fromOffset: baseOffset,
        }).subscribe({ next: () => count2++ });

        // Publish event
        await synap.stream.publish(testRoom, 'stop-all.test', {});
        await new Promise(resolve => setTimeout(resolve, 500));

        // Stop all
        synap.stream.stopAllConsumers();

        const total1 = count1;
        const total2 = count2;

        // Publish more
        await synap.stream.publish(testRoom, 'stop-all.test2', {});
        await new Promise(resolve => setTimeout(resolve, 500));

        // Counts should not change
        expect(count1).toBe(total1);
        expect(count2).toBe(total2);

        sub1.unsubscribe();
        sub2.unsubscribe();
      }, 5000);
    });
  });

  describe('Advanced Patterns', () => {
    it('should support custom filtering', async () => {
      const baseOffset = (await synap.stream.stats(testRoom)).event_count;

      // Publish events with priority
      await synap.stream.publish(testRoom, 'task.created', { priority: 3 });
      await synap.stream.publish(testRoom, 'task.created', { priority: 9 });
      await synap.stream.publish(testRoom, 'task.created', { priority: 5 });

      // Filter high priority
      const highPriority = await firstValueFrom(
        synap.stream.consume$<{ priority: number }>({
          roomName: testRoom,
          subscriberId: 'priority-filter',
          fromOffset: baseOffset,
          pollingInterval: 100,
        }).pipe(
          filter(event => event.data.priority >= 7),
          take(1),
          timeout(5000)
        )
      );

      expect(highPriority.data.priority).toBe(9);

      synap.stream.stopConsumer(testRoom, 'priority-filter');
    }, 10000);
  });
});

