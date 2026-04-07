/**
 * Synap TypeScript SDK - Type Definitions
 * 
 * Core types for the Synap client SDK following the StreamableHTTP protocol.
 */

// ==================== STREAMABLE HTTP PROTOCOL ====================

/**
 * StreamableHTTP request envelope
 */
export interface SynapRequest {
  /** Command to execute (e.g., "kv.set", "queue.publish") */
  command: string;
  /** Unique request identifier (UUID v4) */
  request_id: string;
  /** Command parameters/payload */
  payload: Record<string, any>;
}

/**
 * StreamableHTTP response envelope
 */
export interface SynapResponse<T = any> {
  /** Whether the operation succeeded */
  success: boolean;
  /** Matching request identifier */
  request_id: string;
  /** Response payload (if successful) */
  payload?: T;
  /** Error message (if failed) */
  error?: string;
}

// ==================== CLIENT CONFIGURATION ====================

/**
 * Synap client configuration options
 */
export interface SynapClientOptions {
  /** Synap server URL (default: http://localhost:15500) */
  url?: string;
  /** Request timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Enable request/response logging (default: false) */
  debug?: boolean;
  /** Authentication options */
  auth?: AuthOptions;
  /** Retry configuration */
  retry?: RetryOptions;
}

/**
 * Authentication options
 */
export interface AuthOptions {
  /** Authentication type */
  type: 'basic' | 'api_key';
  /** Username (for Basic Auth) */
  username?: string;
  /** Password (for Basic Auth) */
  password?: string;
  /** API Key (for Bearer Token auth) */
  apiKey?: string;
}

/**
 * Retry configuration
 */
export interface RetryOptions {
  /** Maximum number of retry attempts (default: 3) */
  maxRetries?: number;
  /** Initial retry delay in milliseconds (default: 1000) */
  retryDelay?: number;
  /** Exponential backoff multiplier (default: 2) */
  backoffMultiplier?: number;
}

/**
 * Command execution options shared by multiple modules (e.g., transactions)
 */
export interface CommandOptions {
  /** Optional transaction client identifier to associate commands with MULTI/EXEC */
  clientId?: string;
}

// ==================== KEY-VALUE STORE ====================

/**
 * Key-Value SET options
 */
export interface SetOptions {
  /** Time-to-live in seconds */
  ttl?: number;
}

/**
 * Key-Value statistics
 */
export interface KVStats {
  total_keys: number;
  total_memory_bytes: number;
  operations: {
    gets: number;
    sets: number;
    dels: number;
    hits: number;
    misses: number;
  };
  hit_rate: number;
}

/**
 * SCAN result
 */
export interface ScanResult {
  keys: string[];
  count: number;
}

// ==================== QUEUE SYSTEM ====================

/**
 * Queue configuration
 */
export interface QueueConfig {
  /** Maximum queue depth */
  max_depth?: number;
  /** ACK deadline in seconds */
  ack_deadline_secs?: number;
  /** Default max retries */
  default_max_retries?: number;
  /** Default priority (0-9) */
  default_priority?: number;
}

/**
 * Queue message
 */
export interface QueueMessage {
  /** Unique message identifier */
  id: string;
  /** Message payload (base64 or raw bytes) */
  payload: Uint8Array;
  /** Priority (0-9, where 9 is highest) */
  priority: number;
  /** Number of retry attempts */
  retry_count: number;
  /** Maximum retries allowed */
  max_retries: number;
  /** Custom headers */
  headers?: Record<string, string>;
}

/**
 * Publish message options
 */
export interface PublishOptions {
  /** Message priority (0-9, default: 5) */
  priority?: number;
  /** Maximum retry attempts (default: 3) */
  max_retries?: number;
  /** Custom message headers */
  headers?: Record<string, string>;
}

/**
 * Queue statistics
 */
export interface QueueStats {
  depth: number;
  consumers: number;
  published: number;
  consumed: number;
  acked: number;
  nacked: number;
  dead_lettered: number;
}

/**
 * Consumer options for reactive queue consumption
 */
export interface QueueConsumerOptions {
  /** Queue name to consume from */
  queueName: string;
  /** Consumer identifier */
  consumerId: string;
  /** Polling interval in milliseconds (default: 1000) */
  pollingInterval?: number;
  /** Maximum concurrent messages to process (default: 1) */
  concurrency?: number;
  /** Auto-acknowledge messages on success (default: true) */
  autoAck?: boolean;
  /** Auto-nack messages on error (default: true) */
  autoNack?: boolean;
  /** Requeue on nack (default: true) */
  requeueOnNack?: boolean;
}

/**
 * Processed message with metadata
 */
export interface ProcessedMessage<T = any> {
  /** Original queue message */
  message: QueueMessage;
  /** Decoded payload */
  data: T;
  /** Acknowledge the message */
  ack: () => Promise<void>;
  /** Negative acknowledge the message */
  nack: (requeue?: boolean) => Promise<void>;
}

// ==================== EVENT STREAM ====================

/**
 * Event Stream event
 */
export interface StreamEvent {
  /** Event offset in stream */
  offset: number;
  /** Event name/type */
  event: string;
  /** Event data */
  data: any;
  /** Event timestamp */
  timestamp?: string;
}

/**
 * Stream publish options
 */
export interface StreamPublishOptions {
  /** Custom metadata */
  metadata?: Record<string, string>;
}

/**
 * Stream consumer options for reactive consumption
 */
export interface StreamConsumerOptions {
  /** Room name to consume from */
  roomName: string;
  /** Subscriber identifier */
  subscriberId: string;
  /** Start offset (default: 0) */
  fromOffset?: number;
  /** Polling interval in milliseconds (default: 1000) */
  pollingInterval?: number;
}

/**
 * Stream room statistics
 */
export interface StreamStats {
  /** Maximum offset in stream */
  max_offset: number;
  /** Number of subscribers */
  subscribers: number;
  /** Total events published */
  total_events: number;
  /** Total events consumed */
  total_consumed: number;
  /** Room name */
  room: string;
  /** Created timestamp */
  created_at: number;
  /** Last activity timestamp */
  last_activity: number;
}

/**
 * Processed stream event with metadata
 */
export interface ProcessedStreamEvent<T = any> {
  /** Event offset */
  offset: number;
  /** Event name */
  event: string;
  /** Decoded event data */
  data: T;
  /** Event timestamp */
  timestamp?: string;
}

// ==================== PUB/SUB ====================

/**
 * Pub/Sub message
 */
export interface PubSubMessage<T = any> {
  /** Topic the message was published to */
  topic: string;
  /** Message data */
  data: T;
  /** Message timestamp */
  timestamp?: string;
  /** Message ID */
  id?: string;
}

/**
 * Pub/Sub publish options
 */
export interface PubSubPublishOptions {
  /** Message priority */
  priority?: number;
  /** Custom headers */
  headers?: Record<string, string>;
}

/**
 * Pub/Sub subscriber options for reactive consumption
 */
export interface PubSubSubscriberOptions {
  /** Topics to subscribe to (supports wildcards: user.*, *.created) */
  topics: string[];
  /** Subscriber identifier */
  subscriberId?: string;
  /** Auto-reconnect on disconnect (default: true) */
  autoReconnect?: boolean;
  /** Reconnect interval in ms (default: 1000) */
  reconnectInterval?: number;
}

/**
 * Processed pub/sub message with metadata
 */
export interface ProcessedPubSubMessage<T = any> {
  /** Topic */
  topic: string;
  /** Decoded message data */
  data: T;
  /** Message timestamp */
  timestamp?: string;
  /** Message ID */
  id?: string;
}

// ==================== ERROR TYPES ====================

/**
 * Synap error class
 */
export class SynapError extends Error {
  constructor(
    message: string,
    public code?: string,
    public statusCode?: number,
    public requestId?: string
  ) {
    super(message);
    this.name = 'SynapError';
    Object.setPrototypeOf(this, SynapError.prototype);
  }
}

/**
 * Network error (connection failed)
 */
export class NetworkError extends SynapError {
  constructor(message: string, public originalError?: Error) {
    super(message, 'NETWORK_ERROR');
    this.name = 'NetworkError';
    Object.setPrototypeOf(this, NetworkError.prototype);
  }
}

/**
 * Timeout error (request took too long)
 */
export class TimeoutError extends SynapError {
  constructor(message: string, public timeoutMs: number) {
    super(message, 'TIMEOUT_ERROR');
    this.name = 'TimeoutError';
    Object.setPrototypeOf(this, TimeoutError.prototype);
  }
}

/**
 * Server error (server returned error response)
 */
export class ServerError extends SynapError {
  constructor(message: string, statusCode?: number, requestId?: string) {
    super(message, 'SERVER_ERROR', statusCode, requestId);
    this.name = 'ServerError';
    Object.setPrototypeOf(this, ServerError.prototype);
  }
}

// ==================== UTILITY TYPES ====================

/**
 * JSON-serializable value
 */
export type JSONValue =
  | string
  | number
  | boolean
  | null
  | JSONValue[]
  | { [key: string]: JSONValue };

/**
 * Serializable payload
 */
export type Payload = Record<string, JSONValue>;

// ==================== HYPERLOGLOG ====================

export interface HyperLogLogStats {
  total_hlls: number;
  pfadd_count: number;
  pfcount_count: number;
  pfmerge_count: number;
  total_cardinality: number;
}

