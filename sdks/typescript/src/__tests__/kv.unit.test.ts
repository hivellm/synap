/**
 * KV Store Unit Tests - Additional Coverage
 * Unit tests to cover uncovered lines in kv.ts
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { KVStore } from '../kv';
import { createMockClient } from './__mocks__/client.mock';
import type { SynapClient } from '../client';

describe('KVStore (Unit Tests - Additional Coverage)', () => {
  let mockClient: SynapClient;
  let kv: KVStore;

  beforeEach(() => {
    mockClient = createMockClient();
    kv = new KVStore(mockClient);
  });

  describe('mdel() - Lines 121-124', () => {
    it('should delete multiple keys and return count', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ deleted: 3 });

      const result = await kv.mdel(['key1', 'key2', 'key3']);

      expect(result).toBe(3);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.mdel', {
        keys: ['key1', 'key2', 'key3'],
      });
    });

    it('should handle empty array', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ deleted: 0 });

      const result = await kv.mdel([]);

      expect(result).toBe(0);
    });
  });

  describe('scan() - Lines 129-136', () => {
    it('should scan with prefix', async () => {
      const mockResult = {
        keys: ['user:1', 'user:2'],
        cursor: 'next-cursor',
        has_more: true,
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockResult);

      const result = await kv.scan('user:', 50);

      expect(result).toEqual(mockResult);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.scan', {
        prefix: 'user:',
        limit: 50,
      });
    });

    it('should scan without prefix', async () => {
      const mockResult = {
        keys: ['key1', 'key2'],
        cursor: null,
        has_more: false,
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockResult);

      const result = await kv.scan();

      expect(result).toEqual(mockResult);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.scan', {
        limit: 100,
      });
    });

    it('should handle scan with custom limit', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        keys: [],
        cursor: null,
        has_more: false,
      });

      await kv.scan(undefined, 200);

      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.scan', {
        limit: 200,
      });
    });
  });

  describe('expire() - Lines 157-163', () => {
    it('should set expiration time', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ result: true });

      const result = await kv.expire('mykey', 300);

      expect(result).toBe(true);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.expire', {
        key: 'mykey',
        ttl: 300,
      });
    });

    it('should return false if key does not exist', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ result: false });

      const result = await kv.expire('nonexistent', 60);

      expect(result).toBe(false);
    });
  });

  describe('persist() - Lines 176-179', () => {
    it('should remove expiration from key', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ result: true });

      const result = await kv.persist('mykey');

      expect(result).toBe(true);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.persist', {
        key: 'mykey',
      });
    });

    it('should return false if key does not exist', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ result: false });

      const result = await kv.persist('nonexistent');

      expect(result).toBe(false);
    });
  });

  describe('flushdb() - Lines 184-187', () => {
    it('should flush database and return count', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ flushed: 42 });

      const result = await kv.flushdb();

      expect(result).toBe(42);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.flushdb', {});
    });

    it('should return 0 if database is already empty', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ flushed: 0 });

      const result = await kv.flushdb();

      expect(result).toBe(0);
    });
  });

  describe('flushall() - Lines 192-195', () => {
    it('should flush all databases and return count', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ flushed: 100 });

      const result = await kv.flushall();

      expect(result).toBe(100);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.flushall', {});
    });

    it('should return 0 if all databases are empty', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({ flushed: 0 });

      const result = await kv.flushall();

      expect(result).toBe(0);
    });
  });

  describe('Integration Tests for Uncovered Lines', () => {
    it('should chain multiple operations', async () => {
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({ result: true }) // expire
        .mockResolvedValueOnce({ ttl: 60 }) // ttl
        .mockResolvedValueOnce({ result: true }) // persist
        .mockResolvedValueOnce({ ttl: null }); // ttl after persist

      await kv.expire('key', 60);
      await kv.ttl('key');
      await kv.persist('key');
      await kv.ttl('key');

      expect(mockClient.sendCommand).toHaveBeenCalledTimes(4);
    });

    it('should handle scan pagination', async () => {
      vi.mocked(mockClient.sendCommand)
        .mockResolvedValueOnce({
          keys: ['key1', 'key2'],
          cursor: 'page2',
          has_more: true,
        })
        .mockResolvedValueOnce({
          keys: ['key3', 'key4'],
          cursor: null,
          has_more: false,
        });

      const page1 = await kv.scan('prefix:', 2);
      expect(page1.has_more).toBe(true);

      const page2 = await kv.scan('prefix:', 2);
      expect(page2.has_more).toBe(false);
    });
  });
});

