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
  }
}

/**
 * Timeout error (request took too long)
 */
export class TimeoutError extends SynapError {
  constructor(message: string, public timeoutMs: number) {
    super(message, 'TIMEOUT_ERROR');
    this.name = 'TimeoutError';
  }
}

/**
 * Server error (server returned error response)
 */
export class ServerError extends SynapError {
  constructor(message: string, statusCode?: number, requestId?: string) {
    super(message, 'SERVER_ERROR', statusCode, requestId);
    this.name = 'ServerError';
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

