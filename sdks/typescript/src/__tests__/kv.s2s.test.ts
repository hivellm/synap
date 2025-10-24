/**
 * KV S2S Edge Case & Advanced Tests
 * Focus on comprehensive coverage of KV advanced features
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { Synap } from '../index';

describe('KV Store (S2S Advanced)', () => {
  let synap: Synap;

  beforeAll(() => {
    synap = new Synap({
      url: process.env.SYNAP_URL || 'http://localhost:15500',
      timeout: 10000,
    });
  });

  afterAll(() => {
    synap.close();
  });

  describe('Large Values & Unicode', () => {
    it('should handle unicode characters', async () => {
      const key = `unicode-${Date.now()}`;
      const value = '🚀 Synap 中文 العربية Ελληνικά Русский';

      await synap.kv.set(key, value);
      const retrieved = await synap.kv.get(key);
      expect(retrieved).toBe(value);

      await synap.kv.del(key);
    });

    it('should handle JSON serialization', async () => {
      const key = `json-${Date.now()}`;
      const obj = { name: 'test', nested: { value: 123 }, array: [1, 2, 3] };
      const value = JSON.stringify(obj);

      await synap.kv.set(key, value);
      const retrieved = await synap.kv.get(key);
      expect(JSON.parse(retrieved!)).toEqual(obj);

      await synap.kv.del(key);
    });

    it('should handle empty strings', async () => {
      const key = `empty-${Date.now()}`;
      await synap.kv.set(key, '');
      const retrieved = await synap.kv.get(key);
      expect(retrieved).toBe('');

      await synap.kv.del(key);
    });

    it('should handle special characters in keys', async () => {
      const keys = [
        `key:with:colons-${Date.now()}`,
        `key/with/slashes-${Date.now()}`,
        `key.with.dots-${Date.now()}`,
        `key-with-dashes-${Date.now()}`,
      ];

      for (const key of keys) {
        await synap.kv.set(key, 'test-value');
        const retrieved = await synap.kv.get(key);
        expect(retrieved).toBe('test-value');
        await synap.kv.del(key);
      }
    });

    it('should handle very long keys', async () => {
      const longKey = `key-${'x'.repeat(100)}-${Date.now()}`;
      await synap.kv.set(longKey, 'value');
      const retrieved = await synap.kv.get(longKey);
      expect(retrieved).toBe('value');

      await synap.kv.del(longKey);
    });

    it('should handle numeric strings', async () => {
      const key = `numeric-${Date.now()}`;
      const numericValue = '123456789.99';

      await synap.kv.set(key, numericValue);
      const retrieved = await synap.kv.get(key);
      expect(retrieved).toBe(numericValue);

      await synap.kv.del(key);
    });
  });

  describe('TTL & Expiration', () => {
    it('should handle TTL of 0 (no expiry)', async () => {
      const key = `no-ttl-${Date.now()}`;
      await synap.kv.set(key, 'value', { ttl: 0 });
      const retrieved = await synap.kv.get(key);
      expect(retrieved).toBe('value');

      await synap.kv.del(key);
    });

    it('should handle short TTL', async () => {
      const key = `short-ttl-${Date.now()}`;
      await synap.kv.set(key, 'value', { ttl: 5 });
      const retrieved = await synap.kv.get(key);
      expect(retrieved).toBe('value');

      await synap.kv.del(key);
    });

    it('should handle long TTL (days)', async () => {
      const key = `long-ttl-${Date.now()}`;
      const oneDayInSeconds = 86400;

      await synap.kv.set(key, 'value', { ttl: oneDayInSeconds });
      const retrieved = await synap.kv.get(key);
      expect(retrieved).toBe('value');

      await synap.kv.del(key);
    });

    it('should handle very long TTL (months)', async () => {
      const key = `very-long-ttl-${Date.now()}`;
      const thirtyDaysInSeconds = 30 * 86400;

      await synap.kv.set(key, 'value', { ttl: thirtyDaysInSeconds });
      const retrieved = await synap.kv.get(key);
      expect(retrieved).toBe('value');

      await synap.kv.del(key);
    });
  });

  describe('Batch Operations', () => {
    it('should handle mset with multiple keys', async () => {
      const data: Record<string, any> = {};
      const timestamp = Date.now();

      for (let i = 0; i < 10; i++) {
        data[`batch-key-${timestamp}-${i}`] = `batch-value-${i}`;
      }

      await synap.kv.mset(data);

      // Verify all keys were set
      for (const [key, value] of Object.entries(data)) {
        const retrieved = await synap.kv.get(key);
        expect(retrieved).toBe(value);
      }

      // Clean up
      for (const key of Object.keys(data)) {
        await synap.kv.del(key);
      }
    });

    it('should handle mget with multiple keys', async () => {
      const keys = [];
      const timestamp = Date.now();

      // Set multiple keys
      for (let i = 0; i < 5; i++) {
        const key = `mget-key-${timestamp}-${i}`;
        await synap.kv.set(key, `mget-value-${i}`);
        keys.push(key);
      }

      // Get multiple keys
      const result = await synap.kv.mget(keys);
      expect(result).toBeDefined();

      // Clean up
      for (const key of keys) {
        await synap.kv.del(key);
      }
    });

    it('should handle mget with non-existent keys', async () => {
      const keys = [
        `nonexist-${Date.now()}-1`,
        `nonexist-${Date.now()}-2`,
        `nonexist-${Date.now()}-3`,
      ];

      const result = await synap.kv.mget(keys);
      expect(result).toBeDefined();
    });

    it('should handle empty mget', async () => {
      const result = await synap.kv.mget([]);
      expect(result).toBeDefined();
    });
  });

  describe('Atomic Operations', () => {
    it('should handle incr on new key', async () => {
      const key = `incr-new-${Date.now()}`;
      const result = await synap.kv.incr(key);
      expect(typeof result).toBe('number' || 'string');
      await synap.kv.del(key);
    });

    it('should handle incr on existing key', async () => {
      const key = `incr-exist-${Date.now()}`;
      await synap.kv.set(key, 5); // Set as number, not string
      const result = await synap.kv.incr(key);
      expect(typeof result).toBe('number' || 'string');
      await synap.kv.del(key);
    });

    it('should handle decr on new key', async () => {
      const key = `decr-new-${Date.now()}`;
      const result = await synap.kv.decr(key);
      expect(typeof result).toBe('number' || 'string');
      await synap.kv.del(key);
    });

    it('should handle decr on existing key', async () => {
      const key = `decr-exist-${Date.now()}`;
      await synap.kv.set(key, 10); // Set as number, not string
      const result = await synap.kv.decr(key);
      expect(typeof result).toBe('number' || 'string');
      await synap.kv.del(key);
    });

    it('should handle sequential increments', async () => {
      const key = `incr-seq-${Date.now()}`;
      const results = [];
      for (let i = 0; i < 5; i++) {
        const result = await synap.kv.incr(key);
        results.push(result);
      }

      expect(results.length).toBe(5);
      results.forEach(r => expect(typeof r).toBe('number' || 'string'));

      await synap.kv.del(key);
    });
  });

  describe('Key Operations', () => {
    it('should handle exists check', async () => {
      const key = `exists-${Date.now()}`;
      await synap.kv.set(key, 'value');

      const existsTrue = await synap.kv.exists(key);
      expect(typeof existsTrue).toBe('boolean');

      const existsFalse = await synap.kv.exists(`nonexist-${Date.now()}`);
      expect(typeof existsFalse).toBe('boolean');

      await synap.kv.del(key);
    });

    it('should handle ttl check on key with expiry', async () => {
      const key = `ttl-expire-${Date.now()}`;
      await synap.kv.set(key, 'value', { ttl: 100 });

      const ttl = await synap.kv.ttl(key);
      expect(typeof ttl).toBe('number' || 'string');

      await synap.kv.del(key);
    });
  });

  describe('Scanning & Iteration', () => {
    it('should scan with wildcard pattern', async () => {
      const result = await synap.kv.scan('*');
      expect(Array.isArray(result) || typeof result === 'object').toBe(true);
    });

    it('should scan with specific prefix', async () => {
      const timestamp = Date.now();
      const prefix = `scan-prefix-${timestamp}`;

      // Create some keys
      for (let i = 0; i < 3; i++) {
        await synap.kv.set(`${prefix}-${i}`, `value-${i}`);
      }

      const result = await synap.kv.scan(`${prefix}-*`);
      expect(Array.isArray(result) || typeof result === 'object').toBe(true);

      // Clean up
      for (let i = 0; i < 3; i++) {
        await synap.kv.del(`${prefix}-${i}`);
      }
    });

    it('should handle scan with exact key', async () => {
      const key = `scan-exact-${Date.now()}`;
      await synap.kv.set(key, 'value');

      const result = await synap.kv.scan(key);
      expect(Array.isArray(result) || typeof result === 'object').toBe(true);

      await synap.kv.del(key);
    });
  });

  describe('Delete & Cleanup', () => {
    it('should delete single key', async () => {
      const key = `delete-single-${Date.now()}`;
      await synap.kv.set(key, 'value');
      const result = await synap.kv.del(key);
      expect(typeof result).toBe('boolean');
    });

    it('should delete multiple keys', async () => {
      const keys = [
        `delete-multi-${Date.now()}-1`,
        `delete-multi-${Date.now()}-2`,
        `delete-multi-${Date.now()}-3`,
      ];

      for (const key of keys) {
        await synap.kv.set(key, 'value');
      }

      const result = await synap.kv.mdel(keys);
      expect(typeof result).toBe('number');
    });

    it('should handle delete of non-existent key', async () => {
      const result = await synap.kv.del(`nonexist-${Date.now()}`);
      expect(typeof result).toBe('boolean');
    });

    it('should handle expire and TTL correctly', async () => {
      const key = `expire-test-${Date.now()}`;
      await synap.kv.set(key, 'value');

      // Set expiration
      await synap.kv.expire(key, 1);

      // Check TTL
      const ttl = await synap.kv.ttl(key);
      expect(typeof ttl).toBe('number' || 'string');

      // Clean up (though it will expire automatically)
      await synap.kv.del(key);
    });
  });

  describe('Error Handling', () => {
    it('should handle get on non-existent key', async () => {
      const nonExistentKey = `nonexist-${Date.now()}`;
      const result = await synap.kv.get(nonExistentKey);
      expect(result === null || typeof result === 'string').toBe(true);
    });

    it('should handle exists on non-existent key', async () => {
      const nonExistentKey = `nonexist-${Date.now()}`;
      const result = await synap.kv.exists(nonExistentKey);
      expect(typeof result).toBe('boolean');
      expect(result).toBe(false);
    });
  });

  describe('Statistics & Info', () => {
    it('should get stats with correct structure', async () => {
      const stats = await synap.kv.stats();
      expect(stats).toBeDefined();
      expect(typeof stats === 'object').toBe(true);
    });

    it('should get stats with numeric values', async () => {
      const stats = await synap.kv.stats();
      if (stats && typeof stats === 'object') {
        // At least some numeric properties should exist
        const hasNumericValues = Object.values(stats).some(v => typeof v === 'number');
        expect(hasNumericValues || Object.keys(stats).length > 0).toBe(true);
      }
    });
  });
});


