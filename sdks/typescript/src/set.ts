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
    const response = await this.client.sendCommand<{ added?: number }>('set.add', {
      key,
      members,
    });
    return response?.added ?? 0;
  }

  /**
   * Remove member(s) from set
   */
  async rem(key: string, ...members: string[]): Promise<number> {
    const response = await this.client.sendCommand<{ removed?: number }>('set.rem', {
      key,
      members,
    });
    return response?.removed ?? 0;
  }

  /**
   * Check if member exists in set
   */
  async isMember(key: string, member: string): Promise<boolean> {
    const response = await this.client.sendCommand<{ is_member?: boolean }>('set.ismember', {
      key,
      member,
    });
    return response?.is_member ?? false;
  }

  /**
   * Get all members of set
   */
  async members(key: string): Promise<string[]> {
    const response = await this.client.sendCommand<{ members?: string[] }>('set.members', {
      key,
    });
    return response?.members ?? [];
  }

  /**
   * Get set cardinality (size)
   */
  async card(key: string): Promise<number> {
    const response = await this.client.sendCommand<{ cardinality?: number }>('set.card', {
      key,
    });
    return response?.cardinality ?? 0;
  }

  /**
   * Remove and return random member(s)
   */
  async pop(key: string, count: number = 1): Promise<string[]> {
    const response = await this.client.sendCommand<{ members?: string[] }>('set.pop', {
      key,
      count,
    });
    return response?.members ?? [];
  }

  /**
   * Get random member(s) without removing
   */
  async randMember(key: string, count: number = 1): Promise<string[]> {
    const response = await this.client.sendCommand<{ members?: string[] }>('set.randmember', {
      key,
      count,
    });
    return response?.members ?? [];
  }

  /**
   * Move member from source to destination set
   */
  async move(source: string, destination: string, member: string): Promise<boolean> {
    const response = await this.client.sendCommand<{ moved?: boolean }>('set.move', {
      source,
      destination,
      member,
    });
    return response?.moved ?? false;
  }

  /**
   * Get intersection of sets
   */
  async inter(...keys: string[]): Promise<string[]> {
    const response = await this.client.sendCommand<{ members?: string[] }>('set.inter', {
      keys,
    });
    return response?.members ?? [];
  }

  /**
   * Get union of sets
   */
  async union(...keys: string[]): Promise<string[]> {
    const response = await this.client.sendCommand<{ members?: string[] }>('set.union', {
      keys,
    });
    return response?.members ?? [];
  }

  /**
   * Get difference of sets (first set minus others)
   */
  async diff(...keys: string[]): Promise<string[]> {
    const response = await this.client.sendCommand<{ members?: string[] }>('set.diff', {
      keys,
    });
    return response?.members ?? [];
  }

  /**
   * Store intersection result in destination
   */
  async interStore(destination: string, ...keys: string[]): Promise<number> {
    const response = await this.client.sendCommand<{ cardinality?: number }>('set.interstore', {
      destination,
      keys,
    });
    return response?.cardinality ?? 0;
  }

  /**
   * Store union result in destination
   */
  async unionStore(destination: string, ...keys: string[]): Promise<number> {
    const response = await this.client.sendCommand<{ cardinality?: number }>('set.unionstore', {
      destination,
      keys,
    });
    return response?.cardinality ?? 0;
  }

  /**
   * Store difference result in destination
   */
  async diffStore(destination: string, ...keys: string[]): Promise<number> {
    const response = await this.client.sendCommand<{ cardinality?: number }>('set.diffstore', {
      destination,
      keys,
    });
    return response?.cardinality ?? 0;
  }
}

