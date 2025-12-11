/**
 * Synap TypeScript SDK - Queue System Module
 * 
 * Message queue operations with ACK/NACK support.
 * Includes reactive consumer patterns using RxJS.
 */

import { Observable, Subject, timer, EMPTY, defer } from 'rxjs';
import { 
  switchMap, 
  retry, 
  catchError, 
  filter, 
  takeUntil,
  share,
  mergeMap
} from 'rxjs/operators';
import type { SynapClient } from './client';
import type {
  QueueConfig,
  QueueMessage,
  PublishOptions,
  QueueStats,
  QueueConsumerOptions,
  ProcessedMessage,
} from './types';

/**
 * Queue System client with reactive support
 */
export class QueueManager {
  private stopSignals = new Map<string, Subject<void>>();

  constructor(private client: SynapClient) {}

  /**
   * Create a new queue
   */
  async createQueue(name: string, config?: QueueConfig): Promise<boolean> {
    const result = await this.client.sendCommand<{ success: boolean }>('queue.create', {
      name,
      config: config || {},
    });
    return result.success;
  }

  /**
   * Publish a message to a queue
   */
  async publish(
    queueName: string,
    payload: Uint8Array | string,
    options?: PublishOptions
  ): Promise<string> {
    // Convert payload to array of numbers for JSON serialization
    const payloadBytes =
      typeof payload === 'string'
        ? Array.from(new TextEncoder().encode(payload))
        : Array.from(payload);

    const cmdPayload: Record<string, any> = {
      queue: queueName,
      payload: payloadBytes,
    };

    if (options?.priority !== undefined) {
      cmdPayload.priority = options.priority;
    }

    if (options?.max_retries !== undefined) {
      cmdPayload.max_retries = options.max_retries;
    }

    if (options?.headers) {
      cmdPayload.headers = options.headers;
    }

    const result = await this.client.sendCommand<{ message_id: string }>(
      'queue.publish',
      cmdPayload
    );

    return result.message_id;
  }

  /**
   * Consume a message from a queue
   */
  async consume(queueName: string, consumerId: string): Promise<QueueMessage | null> {
    const result = await this.client.sendCommand<{ message: QueueMessage | null }>(
      'queue.consume',
      {
        queue: queueName,
        consumer_id: consumerId,
      }
    );

    if (!result.message) {
      return null;
    }

    // Convert payload array back to Uint8Array
    const message = result.message;
    if (Array.isArray(message.payload)) {
      message.payload = new Uint8Array(message.payload);
    }

    return message;
  }

  /**
   * Acknowledge message processing (ACK)
   */
  async ack(queueName: string, messageId: string): Promise<boolean> {
    const result = await this.client.sendCommand<{ success: boolean }>('queue.ack', {
      queue: queueName,
      message_id: messageId,
    });
    return result.success;
  }

  /**
   * Negative acknowledge (NACK) - requeue or send to DLQ
   */
  async nack(queueName: string, messageId: string, requeue: boolean = true): Promise<boolean> {
    const result = await this.client.sendCommand<{ success: boolean }>('queue.nack', {
      queue: queueName,
      message_id: messageId,
      requeue,
    });
    return result.success;
  }

  /**
   * Get queue statistics
   */
  async stats(queueName: string): Promise<QueueStats> {
    return this.client.sendCommand<QueueStats>('queue.stats', {
      queue: queueName,
    });
  }

  /**
   * List all queues
   */
  async listQueues(): Promise<string[]> {
    const result = await this.client.sendCommand<{ queues: string[] }>('queue.list', {});
    return result.queues;
  }

  /**
   * Purge all messages from a queue
   */
  async purge(queueName: string): Promise<number> {
    const result = await this.client.sendCommand<{ purged: number }>('queue.purge', {
      queue: queueName,
    });
    return result.purged;
  }

  /**
   * Delete a queue
   */
  async deleteQueue(queueName: string): Promise<boolean> {
    const result = await this.client.sendCommand<{ deleted: boolean }>('queue.delete', {
      queue: queueName,
    });
    return result.deleted;
  }

  /**
   * Helper: Publish a string message
   */
  async publishString(
    queueName: string,
    message: string,
    options?: PublishOptions
  ): Promise<string> {
    return this.publish(queueName, message, options);
  }

  /**
   * Helper: Publish a JSON object
   */
  async publishJSON(
    queueName: string,
    data: any,
    options?: PublishOptions
  ): Promise<string> {
    const json = JSON.stringify(data);
    return this.publish(queueName, json, options);
  }

  /**
   * Helper: Consume and decode as string
   */
  async consumeString(queueName: string, consumerId: string): Promise<{
    message: QueueMessage | null;
    text: string | null;
  }> {
    const message = await this.consume(queueName, consumerId);
    
    if (!message) {
      return { message: null, text: null };
    }

    const text = new TextDecoder().decode(message.payload);
    return { message, text };
  }

  /**
   * Helper: Consume and decode as JSON
   */
  async consumeJSON<T = any>(queueName: string, consumerId: string): Promise<{
    message: QueueMessage | null;
    data: T | null;
  }> {
    const result = await this.consumeString(queueName, consumerId);
    
    if (!result.text) {
      return { message: null, data: null };
    }

    try {
      const data = JSON.parse(result.text) as T;
      return { message: result.message, data };
    } catch (error) {
      throw new Error(`Failed to parse JSON: ${error}`);
    }
  }

  // ==================== REACTIVE METHODS ====================

  /**
   * Create a reactive message consumer as an Observable
   * 
   * @example
   * ```typescript
   * synap.queue.observeMessages({
   *   queueName: 'tasks',
   *   consumerId: 'worker-1',
   *   pollingInterval: 500,
   *   concurrency: 5
   * }).subscribe({
   *   next: async (msg) => {
   *     await processMessage(msg.data);
   *     await msg.ack();
   *   },
   *   error: (err) => console.error('Error:', err)
   * });
   * ```
   */
  observeMessages<T = any>(options: QueueConsumerOptions): Observable<ProcessedMessage<T>> {
    const {
      queueName,
      consumerId,
      pollingInterval = 1000,
      concurrency = 1,
      requeueOnNack = true,
    } = options;

    const stopKey = `${queueName}:${consumerId}`;
    const stopSignal = new Subject<void>();
    this.stopSignals.set(stopKey, stopSignal);

    return timer(0, pollingInterval).pipe(
      takeUntil(stopSignal),
      mergeMap(
        async () => {
          try {
            const result = await this.consumeJSON<T>(queueName, consumerId);
            
            if (!result.message || !result.data) {
              return null;
            }

            const processedMessage: ProcessedMessage<T> = {
              message: result.message,
              data: result.data,
              ack: async () => {
                await this.ack(queueName, result.message!.id);
              },
              nack: async (requeue: boolean = requeueOnNack) => {
                await this.nack(queueName, result.message!.id, requeue);
              },
            };

            return processedMessage;
          } catch (error) {
            console.error(`Error consuming from ${queueName}:`, error);
            return null;
          }
        },
        concurrency
      ),
      filter((msg): msg is ProcessedMessage<T> => msg !== null),
      share()
    ) as Observable<ProcessedMessage<T>>;
  }

  /**
   * Create a reactive message consumer with automatic ACK/NACK handling
   * 
   * @example
   * ```typescript
   * synap.queue.observeMessagesAuto({
   *   queueName: 'tasks',
   *   consumerId: 'worker-1',
   * }).subscribe({
   *   next: async (msg) => {
   *     // Process message - will auto-ACK on success
   *     await processMessage(msg.data);
   *   },
   *   error: (err) => console.error('Error:', err)
   * });
   * ```
   */
  observeMessagesAuto<T = any>(options: QueueConsumerOptions): Observable<ProcessedMessage<T>> {
    const opts = {
      ...options,
      autoAck: true,
      autoNack: true,
    };

    return this.observeMessages<T>(opts);
  }

  /**
   * Create a reactive consumer that processes messages with a handler function
   * 
   * @example
   * ```typescript
   * const subscription = synap.queue.processMessages({
   *   queueName: 'emails',
   *   consumerId: 'email-worker',
   *   concurrency: 10
   * }, async (data) => {
   *   await sendEmail(data);
   * }).subscribe({
   *   next: (result) => console.log('Processed:', result),
   *   error: (err) => console.error('Error:', err)
   * });
   * ```
   */
  processMessages<T = any>(
    options: QueueConsumerOptions,
    handler: (data: T, message: QueueMessage) => Promise<void>
  ): Observable<{ messageId: string; success: boolean; error?: Error }> {
    return this.observeMessages<T>(options).pipe(
      mergeMap(
        async (msg) => {
          try {
            await handler(msg.data, msg.message);
            await msg.ack();
            return { messageId: msg.message.id, success: true };
          } catch (error) {
            await msg.nack();
            return { 
              messageId: msg.message.id, 
              success: false, 
              error: error as Error 
            };
          }
        },
        options.concurrency || 1
      )
    );
  }

  /**
   * Stop a reactive consumer
   * 
   * @param queueName - Queue name
   * @param consumerId - Consumer ID
   */
  stopConsumer(queueName: string, consumerId: string): void {
    const stopKey = `${queueName}:${consumerId}`;
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
   * Create an observable that emits queue statistics at regular intervals
   * 
   * @param queueName - Queue name
   * @param interval - Polling interval in milliseconds (default: 5000)
   * 
   * @example
   * ```typescript
   * synap.queue.observeStats('tasks', 1000).subscribe({
   *   next: (stats) => console.log('Queue depth:', stats.depth),
   * });
   * ```
   */
  observeStats(queueName: string, interval: number = 5000): Observable<QueueStats> {
    return timer(0, interval).pipe(
      switchMap(() => defer(() => this.stats(queueName))),
      retry({
        count: 3,
        delay: 1000,
      }),
      catchError((error: unknown) => {
        console.error(`Error fetching stats for ${queueName}:`, error);
        return EMPTY;
      }),
      share()
    );
  }
}

