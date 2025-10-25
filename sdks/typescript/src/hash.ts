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
    const response = await this.client.sendCommand('hash.get', {
      key,
      field,
    });
    return response.payload?.value || null;
  }

  /**
   * Get all fields and values from hash
   */
  async getAll(key: string): Promise<Record<string, string>> {
    const response = await this.client.sendCommand('hash.getall', {
      key,
    });
    return response.payload?.fields || {};
  }

  /**
   * Delete field from hash
   */
  async del(key: string, field: string): Promise<number> {
    const response = await this.client.sendCommand('hash.del', {
      key,
      field,
    });
    return response.payload?.deleted || 0;
  }

  /**
   * Check if field exists in hash
   */
  async exists(key: string, field: string): Promise<boolean> {
    const response = await this.client.sendCommand('hash.exists', {
      key,
      field,
    });
    return response.payload?.exists || false;
  }

  /**
   * Get all field names in hash
   */
  async keys(key: string): Promise<string[]> {
    const response = await this.client.sendCommand('hash.keys', {
      key,
    });
    return response.payload?.fields || [];
  }

  /**
   * Get all values in hash
   */
  async values(key: string): Promise<string[]> {
    const response = await this.client.sendCommand('hash.values', {
      key,
    });
    return response.payload?.values || [];
  }

  /**
   * Get number of fields in hash
   */
  async len(key: string): Promise<number> {
    const response = await this.client.sendCommand('hash.len', {
      key,
    });
    return response.payload?.length || 0;
  }

  /**
   * Set multiple fields in hash
   */
  async mset(key: string, fields: Record<string, string | number>): Promise<boolean> {
    const response = await this.client.sendCommand('hash.mset', {
      key,
      fields: Object.fromEntries(
        Object.entries(fields).map(([k, v]) => [k, String(v)])
      ),
    });
    return response.success || false;
  }

  /**
   * Get multiple fields from hash
   */
  async mget(key: string, fields: string[]): Promise<Record<string, string | null>> {
    const response = await this.client.sendCommand('hash.mget', {
      key,
      fields,
    });
    return response.payload?.values || {};
  }

  /**
   * Increment field value by integer
   */
  async incrBy(key: string, field: string, increment: number): Promise<number> {
    const response = await this.client.sendCommand('hash.incrby', {
      key,
      field,
      increment,
    });
    return response.payload?.value || 0;
  }

  /**
   * Increment field value by float
   */
  async incrByFloat(key: string, field: string, increment: number): Promise<number> {
    const response = await this.client.sendCommand('hash.incrbyfloat', {
      key,
      field,
      increment,
    });
    return response.payload?.value || 0;
  }

  /**
   * Set field only if it doesn't exist
   */
  async setNX(key: string, field: string, value: string | number): Promise<boolean> {
    const response = await this.client.sendCommand('hash.setnx', {
      key,
      field,
      value: String(value),
    });
    return response.payload?.created || false;
  }
}

