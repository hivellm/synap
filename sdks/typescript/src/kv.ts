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
    const result = await this.client.sendCommand<any>('kv.get', { key });
    
    // Server returns the value directly as a string (JSON stringified)
    // or null if not found
    if (result === null || result === undefined) {
      return null;
    }

    // If result is a string, parse it as JSON
    if (typeof result === 'string') {
      try {
        return JSON.parse(result) as T;
      } catch (error) {
        // If not valid JSON, return as-is
        return result as T;
      }
    }

    return result as T;
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
  async mset(entries: Record<string, JSONValue>): Promise<boolean> {
    const pairs = Object.entries(entries).map(([key, value]) => ({ key, value }));
    const result = await this.client.sendCommand<{ success: boolean }>('kv.mset', { pairs });
    return result.success;
  }

  /**
   * Set multiple key-value pairs atomically only if none of the keys exist (MSETNX)
   * 
   * Supports both object format (new) and tuple format (backward compatible)
   * 
   * @example
   * ```typescript
   * // Object format (preferred)
   * await kv.msetnx({ key: 'user:1', value: 'Alice' });
   * 
   * // Multiple pairs (tuple format - backward compatible)
   * await kv.msetnx({ key: 'user:1', value: 'Alice' }, { key: 'user:2', value: 'Bob' });
   * ```
   */
  async msetnx(...pairs: Array<{ key: string; value: JSONValue }>): Promise<boolean> {
    // If single pair, use object format; otherwise use array format
    const payload = pairs.length === 1 
      ? { key: pairs[0].key, value: pairs[0].value }  // Object format
      : { pairs: pairs.map(p => ({ key: p.key, value: p.value })) };  // Array format
    
    const result = await this.client.sendCommand<{ success: boolean }>('kv.msetnx', payload);
    return result.success;
  }

  /**
   * Get multiple values by keys
   */
  async mget<T = JSONValue>(keys: string[]): Promise<Record<string, T | null>> {
    const result = await this.client.sendCommand<{ values: Array<T | null> }>(
      'kv.mget',
      { keys }
    );
    // Convert array to object keyed by the original keys
    const valuesObj: Record<string, T | null> = {};
    keys.forEach((key, index) => {
      valuesObj[key] = result.values[index];
    });
    return valuesObj;
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
    const result = await this.client.sendCommand<{ result: boolean }>('kv.expire', {
      key,
      ttl: seconds,
    });
    return result.result;
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
    const result = await this.client.sendCommand<{ result: boolean }>('kv.persist', { key });
    return result.result;
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

