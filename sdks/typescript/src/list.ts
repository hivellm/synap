/**
 * Synap TypeScript SDK - List Manager
 * 
 * Redis-compatible List data structure operations
 */

import { SynapClient } from './client';

/**
 * List operations manager
 */
export class ListManager {
  constructor(private client: SynapClient) {}

  /**
   * Push element(s) to left (head) of list
   */
  async lpush(key: string, ...values: string[]): Promise<number> {
    const response = await this.client.sendCommand('list.lpush', {
      key,
      values,
    });
    return response.payload?.length || 0;
  }

  /**
   * Push element(s) to right (tail) of list
   */
  async rpush(key: string, ...values: string[]): Promise<number> {
    const response = await this.client.sendCommand('list.rpush', {
      key,
      values,
    });
    return response.payload?.length || 0;
  }

  /**
   * Pop element from left (head) of list
   */
  async lpop(key: string, count: number = 1): Promise<string[]> {
    const response = await this.client.sendCommand('list.lpop', {
      key,
      count,
    });
    return response.payload?.values || [];
  }

  /**
   * Pop element from right (tail) of list
   */
  async rpop(key: string, count: number = 1): Promise<string[]> {
    const response = await this.client.sendCommand('list.rpop', {
      key,
      count,
    });
    return response.payload?.values || [];
  }

  /**
   * Get range of elements from list
   */
  async range(key: string, start: number = 0, stop: number = -1): Promise<string[]> {
    const response = await this.client.sendCommand('list.range', {
      key,
      start,
      stop,
    });
    return response.payload?.values || [];
  }

  /**
   * Get list length
   */
  async len(key: string): Promise<number> {
    const response = await this.client.sendCommand('list.len', {
      key,
    });
    return response.payload?.length || 0;
  }

  /**
   * Get element at index
   */
  async index(key: string, index: number): Promise<string | null> {
    const response = await this.client.sendCommand('list.index', {
      key,
      index,
    });
    return response.payload?.value || null;
  }

  /**
   * Set element at index
   */
  async set(key: string, index: number, value: string): Promise<boolean> {
    const response = await this.client.sendCommand('list.set', {
      key,
      index,
      value,
    });
    return response.success || false;
  }

  /**
   * Trim list to specified range
   */
  async trim(key: string, start: number, stop: number): Promise<boolean> {
    const response = await this.client.sendCommand('list.trim', {
      key,
      start,
      stop,
    });
    return response.success || false;
  }

  /**
   * Remove elements from list
   */
  async rem(key: string, count: number, value: string): Promise<number> {
    const response = await this.client.sendCommand('list.rem', {
      key,
      count,
      value,
    });
    return response.payload?.removed || 0;
  }

  /**
   * Insert element before/after pivot
   */
  async insert(
    key: string,
    position: 'BEFORE' | 'AFTER',
    pivot: string,
    value: string
  ): Promise<number> {
    const response = await this.client.sendCommand('list.insert', {
      key,
      position: position.toLowerCase(),
      pivot,
      value,
    });
    return response.payload?.length || 0;
  }

  /**
   * Pop from source and push to destination (atomic)
   */
  async rpoplpush(source: string, destination: string): Promise<string | null> {
    const response = await this.client.sendCommand('list.rpoplpush', {
      source,
      destination,
    });
    return response.payload?.value || null;
  }

  /**
   * Find first position of element
   */
  async pos(key: string, element: string, rank: number = 1): Promise<number | null> {
    const response = await this.client.sendCommand('list.pos', {
      key,
      element,
      rank,
    });
    return response.payload?.position ?? null;
  }

  /**
   * Push to left only if list exists
   */
  async lpushx(key: string, ...values: string[]): Promise<number> {
    const response = await this.client.sendCommand('list.lpushx', {
      key,
      values,
    });
    return response.payload?.length || 0;
  }

  /**
   * Push to right only if list exists
   */
  async rpushx(key: string, ...values: string[]): Promise<number> {
    const response = await this.client.sendCommand('list.rpushx', {
      key,
      values,
    });
    return response.payload?.length || 0;
  }
}

