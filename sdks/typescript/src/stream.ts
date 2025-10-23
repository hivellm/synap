/**
 * Synap TypeScript SDK - Event Stream Module
 * 
 * Event stream operations with reactive consumption patterns.
 */

import { Observable, Subject, timer, EMPTY, defer } from 'rxjs';
import { 
  switchMap, 
  retry, 
  catchError, 
  filter, 
  takeUntil,
  share,
  map
} from 'rxjs/operators';
import type { SynapClient } from './client';
import type {
  StreamEvent,
  StreamPublishOptions,
  StreamStats,
  StreamConsumerOptions,
  ProcessedStreamEvent,
} from './types';

/**
 * Event Stream Manager with reactive support
 */
export class StreamManager {
  private stopSignals = new Map<string, Subject<void>>();

  constructor(private client: SynapClient) {}

  /**
   * Create a new stream room
   */
  async createRoom(roomName: string): Promise<boolean> {
    const result = await this.client.sendCommand<{ success: boolean }>('stream.create', {
      room: roomName,
    });
    return result.success;
  }

  /**
   * Publish an event to a stream room
   */
  async publish(
    roomName: string,
    eventName: string,
    data: any,
    options?: StreamPublishOptions
  ): Promise<number> {
    const payload: Record<string, any> = {
      room: roomName,
      event: eventName,
      data,
    };

    if (options?.metadata) {
      payload.metadata = options.metadata;
    }

    const result = await this.client.sendCommand<{ offset: number }>(
      'stream.publish',
      payload
    );

    return result.offset;
  }

  /**
   * Consume events from a stream room (one-time fetch)
   */
  async consume(
    roomName: string,
    subscriberId: string,
    fromOffset: number = 0
  ): Promise<StreamEvent[]> {
    const result = await this.client.sendCommand<{ events: any[] }>(
      'stream.consume',
      {
        room: roomName,
        subscriber_id: subscriberId,
        from_offset: fromOffset,
      }
    );

    const events = result.events || [];
    
    // Parse events - convert byte array data to objects
    return events.map(event => ({
      ...event,
      data: this.parseEventData(event.data),
    }));
  }

  /**
   * Parse event data - handle both byte arrays and objects
   */
  private parseEventData(data: any): any {
    if (Array.isArray(data)) {
      // Convert byte array to string and parse as JSON
      try {
        const text = new TextDecoder().decode(new Uint8Array(data));
        return JSON.parse(text);
      } catch (error) {
        console.error('Failed to parse event data:', error);
        return data;
      }
    }
    return data;
  }

  /**
   * Get stream room statistics
   */
  async stats(roomName: string): Promise<StreamStats> {
    return this.client.sendCommand<StreamStats>('stream.stats', {
      room: roomName,
    });
  }

  /**
   * List all stream rooms
   */
  async listRooms(): Promise<string[]> {
    const result = await this.client.sendCommand<{ rooms: string[] }>('stream.list', {});
    return result.rooms;
  }

  /**
   * Delete a stream room
   */
  async deleteRoom(roomName: string): Promise<boolean> {
    const result = await this.client.sendCommand<{ deleted: any }>('stream.delete', {
      room: roomName,
    });
    // Server returns room name as confirmation, treat as success
    return !!result.deleted;
  }

  // ==================== REACTIVE METHODS ====================

  /**
   * Create a reactive event consumer as an Observable
   * 
   * @example
   * ```typescript
   * synap.stream.consume$({
   *   roomName: 'chat-room',
   *   subscriberId: 'user-123',
   *   fromOffset: 0,
   *   pollingInterval: 500
   * }).subscribe({
   *   next: (event) => {
   *     console.log('Event:', event.event, event.data);
   *   }
   * });
   * ```
   */
  consume$<T = any>(options: StreamConsumerOptions): Observable<ProcessedStreamEvent<T>> {
    const {
      roomName,
      subscriberId,
      fromOffset = 0,
      pollingInterval = 1000,
    } = options;

    const stopKey = `${roomName}:${subscriberId}`;
    const stopSignal = new Subject<void>();
    this.stopSignals.set(stopKey, stopSignal);

    let currentOffset = fromOffset;

    return timer(0, pollingInterval).pipe(
      takeUntil(stopSignal),
      switchMap(async () => {
        try {
          const events = await this.consume(roomName, subscriberId, currentOffset);
          
          if (events.length > 0) {
            // Update offset to last event + 1
            currentOffset = events[events.length - 1].offset + 1;
          }
          
          return events;
        } catch (error) {
          console.error(`Error consuming from stream ${roomName}:`, error);
          return [];
        }
      }),
      // Flatten array of events into individual emissions
      switchMap((events) => events),
      map((event): ProcessedStreamEvent<T> => ({
        offset: event.offset,
        event: event.event,
        data: event.data as T,
        timestamp: event.timestamp,
      })),
      share()
    ) as Observable<ProcessedStreamEvent<T>>;
  }

  /**
   * Create a reactive consumer that filters by event name
   * 
   * @example
   * ```typescript
   * synap.stream.consumeEvent$({
   *   roomName: 'notifications',
   *   subscriberId: 'user-1',
   *   eventName: 'user.created'
   * }).subscribe({
   *   next: (event) => console.log('User created:', event.data)
   * });
   * ```
   */
  consumeEvent$<T = any>(
    options: StreamConsumerOptions & { eventName: string }
  ): Observable<ProcessedStreamEvent<T>> {
    const { eventName, ...consumeOptions } = options;
    
    return this.consume$<T>(consumeOptions).pipe(
      filter((event) => event.event === eventName)
    );
  }

  /**
   * Stop a reactive consumer
   * 
   * @param roomName - Room name
   * @param subscriberId - Subscriber ID
   */
  stopConsumer(roomName: string, subscriberId: string): void {
    const stopKey = `${roomName}:${subscriberId}`;
    const stopSignal = this.stopSignals.get(stopKey);
    
    if (stopSignal) {
      stopSignal.next();
      stopSignal.complete();
      this.stopSignals.delete(stopKey);
    }
  }

  /**
   * Stop all reactive consumers
   */
  stopAllConsumers(): void {
    this.stopSignals.forEach((signal) => {
      signal.next();
      signal.complete();
    });
    this.stopSignals.clear();
  }

  /**
   * Create an observable that emits stream statistics at regular intervals
   * 
   * @param roomName - Room name
   * @param interval - Polling interval in milliseconds (default: 5000)
   * 
   * @example
   * ```typescript
   * synap.stream.stats$('chat-room', 3000).subscribe({
   *   next: (stats) => console.log('Events:', stats.event_count),
   * });
   * ```
   */
  stats$(roomName: string, interval: number = 5000): Observable<StreamStats> {
    return timer(0, interval).pipe(
      switchMap(() => defer(() => this.stats(roomName))),
      retry({
        count: 3,
        delay: 1000,
      }),
      catchError((error: unknown) => {
        console.error(`Error fetching stats for ${roomName}:`, error);
        return EMPTY;
      }),
      share()
    );
  }

  /**
   * Helper: Publish a typed event
   */
  async publishEvent<T>(
    roomName: string,
    eventName: string,
    data: T,
    options?: StreamPublishOptions
  ): Promise<number> {
    return this.publish(roomName, eventName, data, options);
  }
}

