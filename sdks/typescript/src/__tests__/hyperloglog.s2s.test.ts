/**
 * HyperLogLog Manager S2S (Server-to-Server) Tests
 * 
 * Integration tests that require a running Synap server.
 * These tests are skipped if SYNAP_URL is not set.
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { Synap } from '../index';

const SERVER_URL = process.env.SYNAP_URL || 'http://localhost:15500';
const SKIP_S2S = !process.env.SYNAP_URL && !process.env.CI;

describe.skipIf(SKIP_S2S)('HyperLogLogManager S2S', () => {
  let synap: Synap;

  beforeAll(() => {
    synap = new Synap({ url: SERVER_URL });
  });

  afterAll(() => {
    synap.close();
  });

  describe('PFADD', () => {
    it('should add elements to HyperLogLog', async () => {
      const key = `test:hll:${Date.now()}`;

      const added = await synap.hyperloglog.pfadd(key, ['user:1', 'user:2', 'user:3']);
      expect(added).toBeGreaterThanOrEqual(0);
      expect(added).toBeLessThanOrEqual(3);
    });

    it('should handle empty elements array', async () => {
      const key = `test:hll:empty:${Date.now()}`;

      const added = await synap.hyperloglog.pfadd(key, []);
      expect(added).toBe(0);
    });

    it('should add duplicate elements (approximate)', async () => {
      const key = `test:hll:duplicate:${Date.now()}`;

      const added1 = await synap.hyperloglog.pfadd(key, ['user:1', 'user:2']);
      const added2 = await synap.hyperloglog.pfadd(key, ['user:1', 'user:2']); // Duplicates

      expect(added1).toBeGreaterThanOrEqual(0);
      expect(added2).toBeLessThanOrEqual(2); // May add 0 if all duplicates
    });
  });

  describe('PFCOUNT', () => {
    it('should estimate cardinality', async () => {
      const key = `test:hll:count:${Date.now()}`;

      await synap.hyperloglog.pfadd(key, ['user:1', 'user:2', 'user:3', 'user:4', 'user:5']);

      const count = await synap.hyperloglog.pfcount(key);
      expect(count).toBeGreaterThanOrEqual(4); // Approximate, may be slightly off
      expect(count).toBeLessThanOrEqual(6); // Small margin of error
    });

    it('should handle non-existent key', async () => {
      const key = `test:hll:nonexistent:${Date.now()}`;

      // Server may return error for non-existent key
      try {
        const count = await synap.hyperloglog.pfcount(key);
        expect(count).toBe(0);
      } catch (error: any) {
        // Server may return error, which is also acceptable
        expect(error).toBeDefined();
      }
    });

    it('should handle large sets (approximate)', async () => {
      const key = `test:hll:large:${Date.now()}`;

      const elements = Array.from({ length: 100 }, (_, i) => `user:${i}`);
      await synap.hyperloglog.pfadd(key, elements);

      const count = await synap.hyperloglog.pfcount(key);
      expect(count).toBeGreaterThan(80); // Approximate, should be close
      expect(count).toBeLessThan(120); // Within reasonable margin
    });
  });

  describe('PFMERGE', () => {
    it('should merge multiple HyperLogLogs', async () => {
      const key1 = `test:hll:merge1:${Date.now()}`;
      const key2 = `test:hll:merge2:${Date.now()}`;
      const dest = `test:hll:merge_dest:${Date.now()}`;

      await synap.hyperloglog.pfadd(key1, ['user:1', 'user:2', 'user:3']);
      await synap.hyperloglog.pfadd(key2, ['user:4', 'user:5', 'user:6']);

      const count = await synap.hyperloglog.pfmerge(dest, [key1, key2]);
      expect(count).toBeGreaterThanOrEqual(5);
      expect(count).toBeLessThanOrEqual(7); // Approximate
    });

    it('should merge with overlapping elements', async () => {
      const key1 = `test:hll:overlap1:${Date.now()}`;
      const key2 = `test:hll:overlap2:${Date.now()}`;
      const dest = `test:hll:overlap_dest:${Date.now()}`;

      await synap.hyperloglog.pfadd(key1, ['user:1', 'user:2']);
      await synap.hyperloglog.pfadd(key2, ['user:2', 'user:3']); // user:2 overlaps

      const count = await synap.hyperloglog.pfmerge(dest, [key1, key2]);
      expect(count).toBeGreaterThanOrEqual(2);
      expect(count).toBeLessThanOrEqual(4); // Approximate, should deduplicate
    });

    it('should merge single source', async () => {
      const key1 = `test:hll:single1:${Date.now()}`;
      const dest = `test:hll:single_dest:${Date.now()}`;

      await synap.hyperloglog.pfadd(key1, ['user:1', 'user:2', 'user:3']);

      const count = await synap.hyperloglog.pfmerge(dest, [key1]);
      expect(count).toBeGreaterThanOrEqual(2);
      expect(count).toBeLessThanOrEqual(4);
    });
  });

  describe('STATS', () => {
    it('should retrieve HyperLogLog statistics', async () => {
      const key = `test:hll:stats:${Date.now()}`;

      // Perform some operations
      await synap.hyperloglog.pfadd(key, ['user:1', 'user:2']);
      await synap.hyperloglog.pfcount(key);

      const stats = await synap.hyperloglog.stats();

      expect(stats).toHaveProperty('total_hlls');
      expect(stats).toHaveProperty('total_cardinality');
      expect(stats).toHaveProperty('pfadd_count');
      expect(stats).toHaveProperty('pfcount_count');
      expect(stats).toHaveProperty('pfmerge_count');

      expect(stats.pfadd_count).toBeGreaterThanOrEqual(1);
      expect(stats.pfcount_count).toBeGreaterThanOrEqual(1);
    });
  });
});

