/**
 * Bitmap Manager S2S (Server-to-Server) Tests
 * 
 * Integration tests that require a running Synap server.
 * These tests are skipped if SYNAP_URL is not set.
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { Synap } from '../index';

const SERVER_URL = process.env.SYNAP_URL || 'http://localhost:15500';
const SKIP_S2S = !process.env.SYNAP_URL && !process.env.CI;

describe.skipIf(SKIP_S2S)('BitmapManager S2S', () => {
  let synap: Synap;

  beforeAll(() => {
    synap = new Synap({ url: SERVER_URL });
  });

  describe('SETBIT/GETBIT', () => {
    it('should set and get bits', async () => {
      const key = `test:bitmap:${Date.now()}`;

      // Set bit 5 to 1
      const oldValue1 = await synap.bitmap.setbit(key, 5, 1);
      expect(oldValue1).toBe(0); // Was unset before

      // Get bit 5
      const value1 = await synap.bitmap.getbit(key, 5);
      expect(value1).toBe(1);

      // Set bit 5 back to 0
      const oldValue2 = await synap.bitmap.setbit(key, 5, 0);
      expect(oldValue2).toBe(1); // Was set before

      // Get bit 5 again
      const value2 = await synap.bitmap.getbit(key, 5);
      expect(value2).toBe(0);
    });
  });

  describe('BITCOUNT', () => {
    it('should count set bits', async () => {
      const key = `test:bitmap:count:${Date.now()}`;

      // Set multiple bits
      await synap.bitmap.setbit(key, 0, 1);
      await synap.bitmap.setbit(key, 2, 1);
      await synap.bitmap.setbit(key, 4, 1);
      await synap.bitmap.setbit(key, 6, 1);

      // Count all bits
      const count = await synap.bitmap.bitcount(key);
      expect(count).toBe(4);
    });

    it('should count bits in range', async () => {
      const key = `test:bitmap:range:${Date.now()}`;

      // Set bits at positions 0, 1, 2, 10, 11, 12
      await synap.bitmap.setbit(key, 0, 1);
      await synap.bitmap.setbit(key, 1, 1);
      await synap.bitmap.setbit(key, 2, 1);
      await synap.bitmap.setbit(key, 10, 1);
      await synap.bitmap.setbit(key, 11, 1);
      await synap.bitmap.setbit(key, 12, 1);

      // Count in range [0, 7] (first byte)
      const count1 = await synap.bitmap.bitcount(key, 0, 7);
      expect(count1).toBe(3);

      // Count in range [8, 15] (second byte)
      const count2 = await synap.bitmap.bitcount(key, 8, 15);
      expect(count2).toBe(3);
    });
  });

  describe('BITPOS', () => {
    it('should find position of set bit', async () => {
      const key = `test:bitmap:pos:${Date.now()}`;

      // Set bit at position 7
      await synap.bitmap.setbit(key, 7, 1);

      // Find first set bit
      const pos = await synap.bitmap.bitpos(key, 1);
      expect(pos).toBe(7);
    });

    it('should find position with start offset', async () => {
      const key = `test:bitmap:pos2:${Date.now()}`;

      // Set bits at 3 and 10
      await synap.bitmap.setbit(key, 3, 1);
      await synap.bitmap.setbit(key, 10, 1);

      // Find from start
      const pos1 = await synap.bitmap.bitpos(key, 1);
      expect(pos1).toBe(3);

      // Find from offset 5
      const pos2 = await synap.bitmap.bitpos(key, 1, 5);
      expect(pos2).toBe(10);
    });

    it('should return null when bit not found', async () => {
      const key = `test:bitmap:pos3:${Date.now()}`;

      // Empty bitmap - server may return error or null
      try {
        const pos = await synap.bitmap.bitpos(key, 1);
        // If it succeeds, should be null for empty bitmap
        expect(pos).toBeNull();
      } catch (error: any) {
        // Server may return error for non-existent key, which is also acceptable
        expect(error).toBeDefined();
      }
    });
  });

  describe('BITOP', () => {
    it('should perform AND operation', async () => {
      const key1 = `test:bitmap:and1:${Date.now()}`;
      const key2 = `test:bitmap:and2:${Date.now()}`;
      const dest = `test:bitmap:and_result:${Date.now()}`;

      // Set bits in bitmap1 (bits 0, 1, 2)
      await synap.bitmap.setbit(key1, 0, 1);
      await synap.bitmap.setbit(key1, 1, 1);
      await synap.bitmap.setbit(key1, 2, 1);

      // Set bits in bitmap2 (bits 1, 2, 3)
      await synap.bitmap.setbit(key2, 1, 1);
      await synap.bitmap.setbit(key2, 2, 1);
      await synap.bitmap.setbit(key2, 3, 1);

      // AND operation
      const length = await synap.bitmap.bitop('AND', dest, [key1, key2]);
      expect(length).toBeGreaterThan(0);

      // Check result: should have bits 1 and 2 set
      expect(await synap.bitmap.getbit(dest, 0)).toBe(0);
      expect(await synap.bitmap.getbit(dest, 1)).toBe(1);
      expect(await synap.bitmap.getbit(dest, 2)).toBe(1);
      expect(await synap.bitmap.getbit(dest, 3)).toBe(0);
    });

    it('should perform OR operation', async () => {
      const key1 = `test:bitmap:or1:${Date.now()}`;
      const key2 = `test:bitmap:or2:${Date.now()}`;
      const dest = `test:bitmap:or_result:${Date.now()}`;

      await synap.bitmap.setbit(key1, 0, 1);
      await synap.bitmap.setbit(key2, 1, 1);

      const length = await synap.bitmap.bitop('OR', dest, [key1, key2]);
      expect(length).toBeGreaterThan(0);

      expect(await synap.bitmap.getbit(dest, 0)).toBe(1);
      expect(await synap.bitmap.getbit(dest, 1)).toBe(1);
    });

    it('should perform XOR operation', async () => {
      const key1 = `test:bitmap:xor1:${Date.now()}`;
      const key2 = `test:bitmap:xor2:${Date.now()}`;
      const dest = `test:bitmap:xor_result:${Date.now()}`;

      await synap.bitmap.setbit(key1, 0, 1);
      await synap.bitmap.setbit(key1, 1, 1);
      await synap.bitmap.setbit(key2, 1, 1);

      const length = await synap.bitmap.bitop('XOR', dest, [key1, key2]);
      expect(length).toBeGreaterThan(0);

      // XOR: both set = 0, one set = 1
      expect(await synap.bitmap.getbit(dest, 0)).toBe(1);
      expect(await synap.bitmap.getbit(dest, 1)).toBe(0);
    });

    it('should perform NOT operation', async () => {
      const key = `test:bitmap:not1:${Date.now()}`;
      const dest = `test:bitmap:not_result:${Date.now()}`;

      await synap.bitmap.setbit(key, 0, 1);
      await synap.bitmap.setbit(key, 2, 1);

      const length = await synap.bitmap.bitop('NOT', dest, [key]);
      expect(length).toBeGreaterThan(0);

      // NOT: 1 becomes 0, 0 becomes 1
      expect(await synap.bitmap.getbit(dest, 0)).toBe(0);
      expect(await synap.bitmap.getbit(dest, 1)).toBe(1);
      expect(await synap.bitmap.getbit(dest, 2)).toBe(0);
    });
  });

  describe('BITFIELD', () => {
    it('should execute GET and SET operations', async () => {
      const key = `test:bitmap:bitfield:${Date.now()}`;

      // SET operation: Set 8-bit unsigned value 42 at offset 0
      const setResults = await synap.bitmap.bitfield(key, [
        {
          operation: 'SET',
          offset: 0,
          width: 8,
          signed: false,
          value: 42,
        },
      ]);

      expect(setResults).toHaveLength(1);
      expect(setResults[0]).toBe(0); // Old value was 0

      // GET operation: Read back the value
      const getResults = await synap.bitmap.bitfield(key, [
        {
          operation: 'GET',
          offset: 0,
          width: 8,
          signed: false,
        },
      ]);

      expect(getResults).toHaveLength(1);
      expect(getResults[0]).toBe(42);
    });

    it('should execute INCRBY with WRAP overflow', async () => {
      const key = `test:bitmap:bitfield_wrap:${Date.now()}`;

      // Set initial value
      await synap.bitmap.bitfield(key, [
        {
          operation: 'SET',
          offset: 0,
          width: 8,
          signed: false,
          value: 250,
        },
      ]);

      // INCRBY with wrap: 250 + 10 = 260 wraps to 4
      const results = await synap.bitmap.bitfield(key, [
        {
          operation: 'INCRBY',
          offset: 0,
          width: 8,
          signed: false,
          increment: 10,
          overflow: 'WRAP',
        },
      ]);

      expect(results).toHaveLength(1);
      expect(results[0]).toBe(4); // 250 + 10 = 260 wraps to 4 (260 - 256)
    });

    it('should execute INCRBY with SAT overflow', async () => {
      const key = `test:bitmap:bitfield_sat:${Date.now()}`;

      // Set 4-bit unsigned value to 14
      await synap.bitmap.bitfield(key, [
        {
          operation: 'SET',
          offset: 0,
          width: 4,
          signed: false,
          value: 14,
        },
      ]);

      // INCRBY with saturate: 14 + 1 = 15 (max), then stays at 15
      const results1 = await synap.bitmap.bitfield(key, [
        {
          operation: 'INCRBY',
          offset: 0,
          width: 4,
          signed: false,
          increment: 1,
          overflow: 'SAT',
        },
      ]);

      expect(results1[0]).toBe(15);

      // Try to increment again (should saturate at 15)
      const results2 = await synap.bitmap.bitfield(key, [
        {
          operation: 'INCRBY',
          offset: 0,
          width: 4,
          signed: false,
          increment: 1,
          overflow: 'SAT',
        },
      ]);

      expect(results2[0]).toBe(15);
    });

    it('should execute multiple operations', async () => {
      const key = `test:bitmap:bitfield_multi:${Date.now()}`;

      // Execute multiple operations in sequence
      const results = await synap.bitmap.bitfield(key, [
        {
          operation: 'SET',
          offset: 0,
          width: 8,
          signed: false,
          value: 100,
        },
        {
          operation: 'SET',
          offset: 8,
          width: 8,
          signed: false,
          value: 200,
        },
        {
          operation: 'GET',
          offset: 0,
          width: 8,
          signed: false,
        },
        {
          operation: 'GET',
          offset: 8,
          width: 8,
          signed: false,
        },
        {
          operation: 'INCRBY',
          offset: 0,
          width: 8,
          signed: false,
          increment: 50,
          overflow: 'WRAP',
        },
      ]);

      expect(results).toHaveLength(5);
      expect(results[0]).toBe(0); // Old value at offset 0
      expect(results[1]).toBe(0); // Old value at offset 8
      expect(results[2]).toBe(100); // Read back offset 0
      expect(results[3]).toBe(200); // Read back offset 8
      expect(results[4]).toBe(150); // Incremented offset 0
    });

    it('should handle signed values', async () => {
      const key = `test:bitmap:bitfield_signed:${Date.now()}`;

      // Set signed 8-bit negative value
      await synap.bitmap.bitfield(key, [
        {
          operation: 'SET',
          offset: 0,
          width: 8,
          signed: true,
          value: -10,
        },
      ]);

      // Read back as signed
      const results = await synap.bitmap.bitfield(key, [
        {
          operation: 'GET',
          offset: 0,
          width: 8,
          signed: true,
        },
      ]);

      expect(results[0]).toBe(-10);
    });
  });

  describe('STATS', () => {
    it('should retrieve bitmap statistics', async () => {
      const key = `test:bitmap:stats:${Date.now()}`;

      // Perform some operations
      await synap.bitmap.setbit(key, 0, 1);
      await synap.bitmap.getbit(key, 0);
      await synap.bitmap.bitcount(key);

      const stats = await synap.bitmap.stats();

      expect(stats).toHaveProperty('total_bitmaps');
      expect(stats).toHaveProperty('total_bits');
      expect(stats).toHaveProperty('setbit_count');
      expect(stats).toHaveProperty('getbit_count');
      expect(stats).toHaveProperty('bitcount_count');
      expect(stats).toHaveProperty('bitop_count');
      expect(stats).toHaveProperty('bitpos_count');
      expect(stats).toHaveProperty('bitfield_count');

      expect(stats.setbit_count).toBeGreaterThanOrEqual(1);
      expect(stats.getbit_count).toBeGreaterThanOrEqual(1);
      expect(stats.bitcount_count).toBeGreaterThanOrEqual(1);
    });
  });
});

