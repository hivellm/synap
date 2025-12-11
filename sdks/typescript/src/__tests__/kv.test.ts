/**
 * KV Store Unit Tests (Mock)
 * Unit tests using mocked client - no server required
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { KVStore } from '../kv';
import { createMockClient } from './__mocks__/client.mock';
import type { SynapClient } from '../client';

describe('KVStore (Unit Tests)', () => {
  let mockClient: SynapClient;
  let kv: KVStore;

  beforeEach(() => {
    mockClient = createMockClient();
    kv = new KVStore(mockClient);
  });

  describe('SET/GET', () => {
    it('should set and get string value', async () => {
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ success: true })
        .mockResolvedValueOnce(JSON.stringify('hello'));

      await kv.set('key', 'hello');
      const value = await kv.get('key');

      expect(value).toBe('hello');
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.set', {
        key: 'key',
        value: 'hello',
      });
    });

    it('should set and get number value', async () => {
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ success: true })
        .mockResolvedValueOnce(JSON.stringify(42));

      await kv.set('num', 42);
      const value = await kv.get<number>('num');

      expect(value).toBe(42);
    });

    it('should set and get object', async () => {
      const obj = { name: 'Alice', age: 30 };
      
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ success: true })
        .mockResolvedValueOnce(JSON.stringify(obj));

      await kv.set('user', obj);
      const value = await kv.get<typeof obj>('user');

      expect(value).toEqual(obj);
    });

    it('should return null for non-existent key', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue(null);

      const value = await kv.get('nonexistent');
      
      expect(value).toBeNull();
    });

    it('should set with TTL', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ success: true });

      await kv.set('session', { userId: 123 }, { ttl: 60 });

      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.set', {
        key: 'session',
        value: { userId: 123 },
        ttl: 60,
      });
    });
  });

  describe('DELETE', () => {
    it('should delete existing key', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ deleted: true });

      const result = await kv.del('key');
      
      expect(result).toBe(true);
    });

    it('should return false for non-existent key', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ deleted: false });

      const result = await kv.del('nonexistent');
      
      expect(result).toBe(false);
    });
  });

  describe('EXISTS', () => {
    it('should check if key exists', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ exists: true });

      const exists = await kv.exists('key');
      
      expect(exists).toBe(true);
    });

    it('should return false for non-existent key', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ exists: false });

      const exists = await kv.exists('nonexistent');
      
      expect(exists).toBe(false);
    });
  });

  describe('INCR/DECR', () => {
    it('should increment value', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ value: 10 });

      const result = await kv.incr('counter', 10);
      
      expect(result).toBe(10);
    });

    it('should decrement value', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ value: 5 });

      const result = await kv.decr('counter', 5);
      
      expect(result).toBe(5);
    });
  });

  describe('Batch Operations', () => {
    it('should MSET multiple keys', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ success: true });

      const entries = {
        'key1': 'value1',
        'key2': 'value2',
        'key3': 'value3',
      };

      const result = await kv.mset(entries);
      
      expect(result).toBe(true);
      // KVStore converts entries to pairs array
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.mset', {
        pairs: [
          { key: 'key1', value: 'value1' },
          { key: 'key2', value: 'value2' },
          { key: 'key3', value: 'value3' },
        ],
      });
    });

    it('should MGET multiple keys', async () => {
      // MGET returns array in same order as keys
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        values: ['value1', 'value2']
      });

      const values = await kv.mget(['key1', 'key2']);
      
      expect(values).toEqual({
        'key1': 'value1',
        'key2': 'value2',
      });
    });

    it('should MDEL multiple keys', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ deleted: 3 });

      const count = await kv.mdel(['key1', 'key2', 'key3']);
      
      expect(count).toBe(3);
    });
  });

  describe('SCAN', () => {
    it('should scan with prefix', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        keys: ['user:1', 'user:2', 'user:3'],
        count: 3,
      });

      const result = await kv.scan('user:');
      
      expect(result.keys).toEqual(['user:1', 'user:2', 'user:3']);
      expect(result.count).toBe(3);
    });

    it('should scan with limit', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        keys: ['key1', 'key2'],
        count: 2,
      });

      const result = await kv.scan('', 2);
      
      expect(result.count).toBe(2);
      // Empty prefix is not sent
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.scan', {
        limit: 2,
      });
    });
  });

  describe('TTL Operations', () => {
    it('should set expiration', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ result: true });

      const result = await kv.expire('key', 60);
      
      expect(result).toBe(true);
    });

    it('should get TTL', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ ttl: 120 });

      const ttl = await kv.ttl('key');
      
      expect(ttl).toBe(120);
    });

    it('should persist key', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ result: true });

      const result = await kv.persist('key');
      
      expect(result).toBe(true);
    });
  });

  describe('Stats', () => {
    it('should get statistics', async () => {
      const mockStats = {
        total_keys: 100,
        total_memory_bytes: 1024000,
        operations: {
          gets: 500,
          sets: 300,
          dels: 50,
          hits: 400,
          misses: 100,
        },
        hit_rate: 0.8,
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockStats);

      const stats = await kv.stats();
      
      expect(stats).toEqual(mockStats);
      expect(stats.hit_rate).toBe(0.8);
    });
  });
});

