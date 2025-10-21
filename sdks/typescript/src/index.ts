/**
 * Synap TypeScript SDK
 * 
 * Official TypeScript/JavaScript client for Synap server.
 * 
 * @example
 * ```typescript
 * import { Synap } from '@hivellm/synap';
 * 
 * const synap = new Synap({ url: 'http://localhost:15500' });
 * 
 * // Key-Value operations
 * await synap.kv.set('user:1', { name: 'Alice', age: 30 });
 * const user = await synap.kv.get('user:1');
 * 
 * // Queue operations
 * await synap.queue.createQueue('jobs');
 * const msgId = await synap.queue.publishString('jobs', 'process-video');
 * const { message, text } = await synap.queue.consumeString('jobs', 'worker-1');
 * await synap.queue.ack('jobs', message.id);
 * ```
 */

import { SynapClient } from './client';
import { KVStore } from './kv';
import { QueueManager } from './queue';

export type { SynapClientOptions, AuthOptions, RetryOptions } from './types';
export type {
  SetOptions,
  KVStats,
  ScanResult,
  QueueConfig,
  QueueMessage,
  PublishOptions,
  QueueStats,
  JSONValue,
} from './types';

export {
  SynapError,
  NetworkError,
  ServerError,
  TimeoutError,
} from './types';

export { SynapClient } from './client';
export { KVStore } from './kv';
export { QueueManager } from './queue';

/**
 * Main Synap client class
 * 
 * Provides access to all Synap subsystems (KV, Queue, etc.)
 */
export class Synap {
  private client: SynapClient;

  /** Key-Value store operations */
  public readonly kv: KVStore;

  /** Queue system operations */
  public readonly queue: QueueManager;

  constructor(options: import('./types').SynapClientOptions = {}) {
    this.client = new SynapClient(options);
    this.kv = new KVStore(this.client);
    this.queue = new QueueManager(this.client);
  }

  /**
   * Ping the server
   */
  async ping(): Promise<boolean> {
    return this.client.ping();
  }

  /**
   * Get server health
   */
  async health(): Promise<{ status: string; service: string; version: string }> {
    return this.client.health();
  }

  /**
   * Close the client
   */
  close(): void {
    this.client.close();
  }

  /**
   * Get the underlying HTTP client (for advanced usage)
   */
  getClient(): SynapClient {
    return this.client;
  }
}

// Default export
export default Synap;

