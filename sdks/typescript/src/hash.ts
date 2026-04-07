/**
 * Synap TypeScript SDK - Hash Manager
 * 
 * Redis-compatible Hash data structure operations
 */

import { SynapClient } from './client';

/**
 * Hash field-value pair
 */
export interface HashField {
  field: string;
  value: string | number;
}

/**
 * Hash operations manager
 */
export class HashManager {
  constructor(private client: SynapClient) {}

  /**
   * Set field in hash
   */
  async set(key: string, field: string, value: string | number): Promise<boolean> {
    const response = await this.client.sendCommand<{ success: boolean }>('hash.set', {
      key,
      field,
      value: String(value),
    });
    return response.success || false;
  }

  /**
   * Get field from hash
   */
  async get(key: string, field: string): Promise<string | null> {
    const response = await this.client.sendCommand<{ value?: string }>('hash.get', {
      key,
      field,
    });
    return response?.value ?? null;
  }

  /**
   * Get all fields and values from hash
   */
  async getAll(key: string): Promise<Record<string, string>> {
    const response = await this.client.sendCommand<{ fields?: Record<string, string> }>('hash.getall', {
      key,
    });
    return response?.fields ?? {};
  }

  /**
   * Delete field from hash
   */
  async del(key: string, field: string): Promise<number> {
    const response = await this.client.sendCommand<{ deleted?: number }>('hash.del', {
      key,
      field,
    });
    return response?.deleted ?? 0;
  }

  /**
   * Check if field exists in hash
   */
  async exists(key: string, field: string): Promise<boolean> {
    const response = await this.client.sendCommand<{ exists?: boolean }>('hash.exists', {
      key,
      field,
    });
    return response?.exists ?? false;
  }

  /**
   * Get all field names in hash
   */
  async keys(key: string): Promise<string[]> {
    const response = await this.client.sendCommand<{ fields?: string[] }>('hash.keys', {
      key,
    });
    return response?.fields ?? [];
  }

  /**
   * Get all values in hash
   */
  async values(key: string): Promise<string[]> {
    const response = await this.client.sendCommand<{ values?: string[] }>('hash.values', {
      key,
    });
    return response?.values ?? [];
  }

  /**
   * Get number of fields in hash
   */
  async len(key: string): Promise<number> {
    const response = await this.client.sendCommand<{ length?: number }>('hash.len', {
      key,
    });
    return response?.length ?? 0;
  }

  /**
   * Set multiple fields in hash
   * 
   * Supports both object format (backward compatible) and array format (Redis-compatible)
   * 
   * @example
   * ```typescript
   * // Object format (backward compatible)
   * await hash.mset('user:1', { name: 'Alice', age: 30 });
   * 
   * // Array format (Redis-compatible)
   * await hash.mset('user:1', [{ field: 'name', value: 'Alice' }, { field: 'age', value: '30' }]);
   * ```
   */
  async mset(key: string, fields: Record<string, string | number> | Array<{ field: string; value: string | number }>): Promise<boolean> {
    let payload: Record<string, any> = { key };
    
    // Check if fields is an array (new format) or object (backward compatible)
    if (Array.isArray(fields)) {
      // Array format: [{"field": "...", "value": "..."}, ...]
      payload.fields = fields.map(f => ({
        field: f.field,
        value: String(f.value)
      }));
    } else {
      // Object format (backward compatible)
      payload.fields = Object.fromEntries(
        Object.entries(fields).map(([k, v]) => [k, String(v)])
      );
    }
    
    const response = await this.client.sendCommand<{ success?: boolean }>('hash.mset', payload);
    return response?.success ?? false;
  }

  /**
   * Get multiple fields from hash
   */
  async mget(key: string, fields: string[]): Promise<Record<string, string | null>> {
    const response = await this.client.sendCommand<{ values?: Record<string, string | null> }>(
      'hash.mget',
      {
        key,
        fields,
      }
    );
    return response?.values ?? {};
  }

  /**
   * Increment field value by integer
   */
  async incrBy(key: string, field: string, increment: number): Promise<number> {
    const response = await this.client.sendCommand<{ value?: number }>('hash.incrby', {
      key,
      field,
      increment,
    });
    return response?.value ?? 0;
  }

  /**
   * Increment field value by float
   */
  async incrByFloat(key: string, field: string, increment: number): Promise<number> {
    const response = await this.client.sendCommand<{ value?: number }>('hash.incrbyfloat', {
      key,
      field,
      increment,
    });
    return response?.value ?? 0;
  }

  /**
   * Set field only if it doesn't exist
   */
  async setNX(key: string, field: string, value: string | number): Promise<boolean> {
    const response = await this.client.sendCommand<{ created?: boolean }>('hash.setnx', {
      key,
      field,
      value: String(value),
    });
    return response?.created ?? false;
  }
}

