/**
 * Synap TypeScript SDK - Key-Value Store Module
 * 
 * Provides key-value operations using the StreamableHTTP protocol.
 */

import type { SynapClient } from './client';
import type { SetOptions, KVStats, ScanResult, JSONValue } from './types';

/**
 * Key-Value Store client
 */
export class KVStore {
  constructor(private client: SynapClient) {}

  /**
   * Set a key-value pair
   */
  async set(key: string, value: JSONValue, options?: SetOptions): Promise<boolean> {
    const payload: Record<string, any> = { key, value };
    
    if (options?.ttl) {
      payload.ttl = options.ttl;
    }

    const result = await this.client.sendCommand<{ success: boolean }>('kv.set', payload);
    return result.success;
  }

  /**
   * Get a value by key
   */
  async get<T = JSONValue>(key: string): Promise<T | null> {
    const result = await this.client.sendCommand<{ value: T | null }>('kv.get', { key });
    return result.value;
  }

  /**
   * Delete a key
   */
  async del(key: string): Promise<boolean> {
    const result = await this.client.sendCommand<{ deleted: boolean }>('kv.del', { key });
    return result.deleted;
  }

  /**
   * Check if a key exists
   */
  async exists(key: string): Promise<boolean> {
    const result = await this.client.sendCommand<{ exists: boolean }>('kv.exists', { key });
    return result.exists;
  }

  /**
   * Increment a numeric value
   */
  async incr(key: string, amount: number = 1): Promise<number> {
    const result = await this.client.sendCommand<{ value: number }>('kv.incr', {
      key,
      amount,
    });
    return result.value;
  }

  /**
   * Decrement a numeric value
   */
  async decr(key: string, amount: number = 1): Promise<number> {
    const result = await this.client.sendCommand<{ value: number }>('kv.decr', {
      key,
      amount,
    });
    return result.value;
  }

  /**
   * Set multiple key-value pairs atomically
   */
  async mset(entries: Record<string, JSONValue>): Promise<number> {
    const result = await this.client.sendCommand<{ count: number }>('kv.mset', { entries });
    return result.count;
  }

  /**
   * Get multiple values by keys
   */
  async mget<T = JSONValue>(keys: string[]): Promise<Record<string, T | null>> {
    const result = await this.client.sendCommand<{ values: Record<string, T | null> }>(
      'kv.mget',
      { keys }
    );
    return result.values;
  }

  /**
   * Delete multiple keys
   */
  async mdel(keys: string[]): Promise<number> {
    const result = await this.client.sendCommand<{ deleted: number }>('kv.mdel', { keys });
    return result.deleted;
  }

  /**
   * Scan keys with optional prefix
   */
  async scan(prefix?: string, limit: number = 100): Promise<ScanResult> {
    const payload: Record<string, any> = { limit };
    if (prefix) {
      payload.prefix = prefix;
    }

    return this.client.sendCommand<ScanResult>('kv.scan', payload);
  }

  /**
   * List all keys matching a pattern
   */
  async keys(pattern: string = '*'): Promise<string[]> {
    const result = await this.client.sendCommand<{ keys: string[] }>('kv.keys', { pattern });
    return result.keys;
  }

  /**
   * Get database size (number of keys)
   */
  async dbsize(): Promise<number> {
    const result = await this.client.sendCommand<{ size: number }>('kv.dbsize', {});
    return result.size;
  }

  /**
   * Set expiration time for a key
   */
  async expire(key: string, seconds: number): Promise<boolean> {
    const result = await this.client.sendCommand<{ success: boolean }>('kv.expire', {
      key,
      seconds,
    });
    return result.success;
  }

  /**
   * Get TTL for a key
   */
  async ttl(key: string): Promise<number | null> {
    const result = await this.client.sendCommand<{ ttl: number | null }>('kv.ttl', { key });
    return result.ttl;
  }

  /**
   * Remove expiration from a key
   */
  async persist(key: string): Promise<boolean> {
    const result = await this.client.sendCommand<{ success: boolean }>('kv.persist', { key });
    return result.success;
  }

  /**
   * Flush current database
   */
  async flushdb(): Promise<number> {
    const result = await this.client.sendCommand<{ flushed: number }>('kv.flushdb', {});
    return result.flushed;
  }

  /**
   * Flush all databases
   */
  async flushall(): Promise<number> {
    const result = await this.client.sendCommand<{ flushed: number }>('kv.flushall', {});
    return result.flushed;
  }

  /**
   * Get store statistics
   */
  async stats(): Promise<KVStats> {
    return this.client.sendCommand<KVStats>('kv.stats', {});
  }
}

