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
 * 
 * // Event Stream operations
 * await synap.stream.createRoom('chat-room');
 * await synap.stream.publish('chat-room', 'message.sent', { text: 'Hello!' });
 * 
 * // Pub/Sub operations
 * await synap.pubsub.publish('user.created', { id: 123, name: 'Alice' });
 * ```
 */

import { SynapClient } from './client';
import { KVStore } from './kv';
import { HashManager } from './hash';
import { ListManager } from './list';
import { SetManager } from './set';
import { QueueManager } from './queue';
import { StreamManager } from './stream';
import { PubSubManager } from './pubsub';

export type { SynapClientOptions, AuthOptions, RetryOptions } from './types';
export type {
  SetOptions,
  KVStats,
  ScanResult,
  QueueConfig,
  QueueMessage,
  PublishOptions,
  QueueStats,
  StreamEvent,
  StreamPublishOptions,
  StreamStats,
  StreamConsumerOptions,
  ProcessedStreamEvent,
  PubSubMessage,
  PubSubPublishOptions,
  PubSubSubscriberOptions,
  ProcessedPubSubMessage,
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
export { HashManager } from './hash';
export { ListManager } from './list';
export { SetManager } from './set';
export { QueueManager } from './queue';
export { StreamManager } from './stream';
export { PubSubManager } from './pubsub';

/**
 * Main Synap client class
 * 
 * Provides access to all Synap subsystems (KV, Queue, Stream, PubSub)
 */
export class Synap {
  private client: SynapClient;

  /** Key-Value store operations */
  public readonly kv: KVStore;

  /** Hash data structure operations */
  public readonly hash: HashManager;

  /** List data structure operations */
  public readonly list: ListManager;

  /** Set data structure operations */
  public readonly set: SetManager;

  /** Queue system operations */
  public readonly queue: QueueManager;

  /** Event Stream operations */
  public readonly stream: StreamManager;

  /** Pub/Sub operations */
  public readonly pubsub: PubSubManager;

  constructor(options: import('./types').SynapClientOptions = {}) {
    this.client = new SynapClient(options);
    this.kv = new KVStore(this.client);
    this.hash = new HashManager(this.client);
    this.list = new ListManager(this.client);
    this.set = new SetManager(this.client);
    this.queue = new QueueManager(this.client);
    this.stream = new StreamManager(this.client);
    this.pubsub = new PubSubManager(this.client);
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

