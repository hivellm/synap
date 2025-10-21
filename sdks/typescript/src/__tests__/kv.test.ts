/**
 * KV Store Module Tests
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { Synap } from '../index';

describe('KVStore', () => {
  let synap: Synap;

  beforeAll(() => {
    synap = new Synap({
      url: process.env.SYNAP_URL || 'http://localhost:15500',
    });
  });

  afterAll(() => {
    synap.close();
  });

  describe('SET/GET operations', () => {
    it('should set and get a string value', async () => {
      await synap.kv.set('test:string', 'hello world');
      const value = await synap.kv.get('test:string');
      expect(value).toBe('hello world');
    });

    it('should set and get a number value', async () => {
      await synap.kv.set('test:number', 42);
      const value = await synap.kv.get<number>('test:number');
      expect(value).toBe(42);
    });

    it('should set and get an object', async () => {
      const obj = { name: 'Alice', age: 30, tags: ['admin', 'user'] };
      await synap.kv.set('test:object', obj);
      const value = await synap.kv.get<typeof obj>('test:object');
      expect(value).toEqual(obj);
    });

    it('should return null for non-existent key', async () => {
      const value = await synap.kv.get('nonexistent:key');
      expect(value).toBeNull();
    });

    it('should set with TTL', async () => {
      await synap.kv.set('test:ttl', 'temporary', { ttl: 1 });
      const exists = await synap.kv.exists('test:ttl');
      expect(exists).toBe(true);
    });
  });

  describe('DELETE operations', () => {
    it('should delete an existing key', async () => {
      await synap.kv.set('test:delete', 'value');
      const deleted = await synap.kv.del('test:delete');
      expect(deleted).toBe(true);

      const value = await synap.kv.get('test:delete');
      expect(value).toBeNull();
    });

    it('should return false when deleting non-existent key', async () => {
      const deleted = await synap.kv.del('nonexistent:key');
      expect(deleted).toBe(false);
    });
  });

  describe('EXISTS operation', () => {
    it('should return true for existing key', async () => {
      await synap.kv.set('test:exists', 'value');
      const exists = await synap.kv.exists('test:exists');
      expect(exists).toBe(true);
    });

    it('should return false for non-existent key', async () => {
      const exists = await synap.kv.exists('nonexistent:key');
      expect(exists).toBe(false);
    });
  });

  describe('INCR/DECR operations', () => {
    it('should increment a value', async () => {
      await synap.kv.set('test:counter', 10);
      const newValue = await synap.kv.incr('test:counter', 5);
      expect(newValue).toBe(15);
    });

    it('should decrement a value', async () => {
      await synap.kv.set('test:counter2', 20);
      const newValue = await synap.kv.decr('test:counter2', 3);
      expect(newValue).toBe(17);
    });
  });

  describe('Batch operations', () => {
    it('should MSET multiple keys', async () => {
      const count = await synap.kv.mset({
        'batch:1': 'value1',
        'batch:2': 'value2',
        'batch:3': 'value3',
      });
      expect(count).toBe(3);
    });

    it('should MGET multiple keys', async () => {
      await synap.kv.mset({
        'multi:a': 'A',
        'multi:b': 'B',
        'multi:c': 'C',
      });

      const values = await synap.kv.mget(['multi:a', 'multi:b', 'multi:c']);
      expect(values).toEqual({
        'multi:a': 'A',
        'multi:b': 'B',
        'multi:c': 'C',
      });
    });

    it('should MDEL multiple keys', async () => {
      await synap.kv.mset({
        'del:1': 'v1',
        'del:2': 'v2',
        'del:3': 'v3',
      });

      const deleted = await synap.kv.mdel(['del:1', 'del:2', 'del:3']);
      expect(deleted).toBe(3);
    });
  });

  describe('SCAN operation', () => {
    it('should scan with prefix', async () => {
      await synap.kv.mset({
        'scan:user:1': 'Alice',
        'scan:user:2': 'Bob',
        'scan:product:1': 'Widget',
      });

      const result = await synap.kv.scan('scan:user:');
      expect(result.keys.length).toBe(2);
      expect(result.keys).toContain('scan:user:1');
      expect(result.keys).toContain('scan:user:2');
    });
  });

  describe('TTL operations', () => {
    it('should set expiration', async () => {
      await synap.kv.set('test:expire', 'temp');
      await synap.kv.expire('test:expire', 60);
      
      const ttl = await synap.kv.ttl('test:expire');
      expect(ttl).toBeGreaterThan(0);
      expect(ttl).toBeLessThanOrEqual(60);
    });

    it('should persist (remove expiration)', async () => {
      await synap.kv.set('test:persist', 'value', { ttl: 60 });
      await synap.kv.persist('test:persist');
      
      const ttl = await synap.kv.ttl('test:persist');
      expect(ttl).toBeNull();
    });
  });

  describe('Stats', () => {
    it('should get store statistics', async () => {
      const stats = await synap.kv.stats();
      
      expect(stats).toHaveProperty('total_keys');
      expect(stats).toHaveProperty('total_memory_bytes');
      expect(stats).toHaveProperty('operations');
      expect(stats).toHaveProperty('hit_rate');
      expect(typeof stats.total_keys).toBe('number');
    });
  });
});

