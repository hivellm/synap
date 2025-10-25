/**
 * Synap TypeScript SDK - Set Manager
 * 
 * Redis-compatible Set data structure operations
 */

import { SynapClient } from './client';

/**
 * Set operations manager
 */
export class SetManager {
  constructor(private client: SynapClient) {}

  /**
   * Add member(s) to set
   */
  async add(key: string, ...members: string[]): Promise<number> {
    const response = await this.client.sendCommand('set.add', {
      key,
      members,
    });
    return response.payload?.added || 0;
  }

  /**
   * Remove member(s) from set
   */
  async rem(key: string, ...members: string[]): Promise<number> {
    const response = await this.client.sendCommand('set.rem', {
      key,
      members,
    });
    return response.payload?.removed || 0;
  }

  /**
   * Check if member exists in set
   */
  async isMember(key: string, member: string): Promise<boolean> {
    const response = await this.client.sendCommand('set.ismember', {
      key,
      member,
    });
    return response.payload?.is_member || false;
  }

  /**
   * Get all members of set
   */
  async members(key: string): Promise<string[]> {
    const response = await this.client.sendCommand('set.members', {
      key,
    });
    return response.payload?.members || [];
  }

  /**
   * Get set cardinality (size)
   */
  async card(key: string): Promise<number> {
    const response = await this.client.sendCommand('set.card', {
      key,
    });
    return response.payload?.cardinality || 0;
  }

  /**
   * Remove and return random member(s)
   */
  async pop(key: string, count: number = 1): Promise<string[]> {
    const response = await this.client.sendCommand('set.pop', {
      key,
      count,
    });
    return response.payload?.members || [];
  }

  /**
   * Get random member(s) without removing
   */
  async randMember(key: string, count: number = 1): Promise<string[]> {
    const response = await this.client.sendCommand('set.randmember', {
      key,
      count,
    });
    return response.payload?.members || [];
  }

  /**
   * Move member from source to destination set
   */
  async move(source: string, destination: string, member: string): Promise<boolean> {
    const response = await this.client.sendCommand('set.move', {
      source,
      destination,
      member,
    });
    return response.payload?.moved || false;
  }

  /**
   * Get intersection of sets
   */
  async inter(...keys: string[]): Promise<string[]> {
    const response = await this.client.sendCommand('set.inter', {
      keys,
    });
    return response.payload?.members || [];
  }

  /**
   * Get union of sets
   */
  async union(...keys: string[]): Promise<string[]> {
    const response = await this.client.sendCommand('set.union', {
      keys,
    });
    return response.payload?.members || [];
  }

  /**
   * Get difference of sets (first set minus others)
   */
  async diff(...keys: string[]): Promise<string[]> {
    const response = await this.client.sendCommand('set.diff', {
      keys,
    });
    return response.payload?.members || [];
  }

  /**
   * Store intersection result in destination
   */
  async interStore(destination: string, ...keys: string[]): Promise<number> {
    const response = await this.client.sendCommand('set.interstore', {
      destination,
      keys,
    });
    return response.payload?.cardinality || 0;
  }

  /**
   * Store union result in destination
   */
  async unionStore(destination: string, ...keys: string[]): Promise<number> {
    const response = await this.client.sendCommand('set.unionstore', {
      destination,
      keys,
    });
    return response.payload?.cardinality || 0;
  }

  /**
   * Store difference result in destination
   */
  async diffStore(destination: string, ...keys: string[]): Promise<number> {
    const response = await this.client.sendCommand('set.diffstore', {
      destination,
      keys,
    });
    return response.payload?.cardinality || 0;
  }
}

