/**
 * Synap TypeScript SDK - Pub/Sub Module
 * 
 * Pub/Sub operations with reactive subscription patterns.
 */

import { Observable, Subject } from 'rxjs';
import {  
  share,
  takeUntil,
} from 'rxjs/operators';
import type { SynapClient } from './client';
import type {
  PubSubPublishOptions,
  PubSubSubscriberOptions,
  ProcessedPubSubMessage,
} from './types';

/**
 * Pub/Sub Manager with reactive support
 */
export class PubSubManager {
  private subscriptions = new Map<string, Subject<void>>();

  constructor(private client: SynapClient) {}

  /**
   * Publish a message to a topic
   */
  async publish(
    topic: string,
    data: any,
    options?: PubSubPublishOptions
  ): Promise<boolean> {
    const cmdPayload: Record<string, any> = {
      topic,
      payload: data,  // ✅ FIX: Use "payload" instead of "data" to match server API
    };

    if (options?.priority !== undefined) {
      cmdPayload.priority = options.priority;
    }

    if (options?.headers) {
      cmdPayload.headers = options.headers;
    }

    const result = await this.client.sendCommand<{ message_id: string; subscribers_matched: number }>(
      'pubsub.publish',
      cmdPayload
    );

    // Return true if publish succeeded (message_id present)
    return !!result.message_id;
  }

  /**
   * Helper: Publish a typed message
   */
  async publishMessage<T>(
    topic: string,
    data: T,
    options?: PubSubPublishOptions
  ): Promise<boolean> {
    return this.publish(topic, data, options);
  }

  // ==================== REACTIVE METHODS ====================

  /**
   * Create a reactive pub/sub subscriber as an Observable
   * 
   * Note: This method creates a simulated reactive subscription using polling.
   * For real-time WebSocket-based subscriptions, consider using the WebSocket API directly.
   * 
   * @example
   * ```typescript
   * synap.pubsub.subscribe({
   *   topics: ['user.created', 'user.updated'],
   *   subscriberId: 'subscriber-1'
   * }).subscribe({
   *   next: (message) => {
   *     console.log('Topic:', message.topic);
   *     console.log('Data:', message.data);
   *   }
   * });
   * ```
   */
  subscribe<T = any>(options: PubSubSubscriberOptions): Observable<ProcessedPubSubMessage<T>> {
    const {
      topics,
      subscriberId = `subscriber-${Date.now()}`,
    } = options;

    const stopKey = `${subscriberId}:${topics.join(',')}`;
    const stopSignal = new Subject<void>();
    this.subscriptions.set(stopKey, stopSignal);

    // Create observable for pub/sub messages
    // This is a simplified implementation using Subject
    // In a real implementation, this would connect to WebSocket
    const messageSubject = new Subject<ProcessedPubSubMessage<T>>();

    // Setup connection (simulated for now)
    // In production, this would open a WebSocket connection
    const setupSubscription = async () => {
      try {
        // Subscribe to topics
        await this.client.sendCommand('pubsub.subscribe', {
          topics,
          subscriber_id: subscriberId,
        });
      } catch (error) {
        console.error(`Error subscribing to topics ${topics.join(', ')}:`, error);
        messageSubject.error(error);
      }
    };

    // Start subscription
    setupSubscription();

    return messageSubject.pipe(
      takeUntil(stopSignal),
      share()
    ) as Observable<ProcessedPubSubMessage<T>>;
  }

  /**
   * Subscribe to a single topic with reactive pattern
   * 
   * @example
   * ```typescript
   * synap.pubsub.subscribeTopic('user.created').subscribe({
   *   next: (message) => console.log('User created:', message.data)
   * });
   * ```
   */
  subscribeTopic<T = any>(topic: string, subscriberId?: string): Observable<ProcessedPubSubMessage<T>> {
    return this.subscribe<T>({
      topics: [topic],
      subscriberId,
    });
  }

  /**
   * Stop a reactive subscription
   * 
   * @param subscriberId - Subscriber ID
   * @param topics - Topics that were subscribed to
   */
  unsubscribe(subscriberId: string, topics: string[]): void {
    const stopKey = `${subscriberId}:${topics.join(',')}`;
    const stopSignal = this.subscriptions.get(stopKey);
    
    if (stopSignal) {
      stopSignal.next();
      stopSignal.complete();
      this.subscriptions.delete(stopKey);
    }

    // Unsubscribe from server
    this.client.sendCommand('pubsub.unsubscribe', {
      subscriber_id: subscriberId,
      topics,
    }).catch((error) => {
      console.error(`Error unsubscribing from topics:`, error);
    });
  }

  /**
   * Stop all reactive subscriptions
   */
  unsubscribeAll(): void {
    this.subscriptions.forEach((signal) => {
      signal.next();
      signal.complete();
    });
    this.subscriptions.clear();
  }

  /**
   * Get statistics for a topic (if supported by server)
   */
  async stats(topic: string): Promise<any> {
    return this.client.sendCommand('pubsub.stats', {
      topic,
    });
  }

  /**
   * List all active topics
   */
  async listTopics(): Promise<string[]> {
    const result = await this.client.sendCommand<{ topics: string[] }>('pubsub.list', {});
    return result.topics;
  }
}

