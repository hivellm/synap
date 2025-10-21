/**
 * Synap TypeScript SDK - Queue System Module
 * 
 * Message queue operations with ACK/NACK support.
 */

import type { SynapClient } from './client';
import type {
  QueueConfig,
  QueueMessage,
  PublishOptions,
  QueueStats,
} from './types';

/**
 * Queue System client
 */
export class QueueManager {
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
}

