/**
 * Bitmap Manager Unit Tests
 * 
 * Tests for Bitmap operations using mocked HTTP client
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { BitmapManager } from '../bitmap';
import { SynapClient } from '../client';

describe('BitmapManager', () => {
  let manager: BitmapManager;
  let mockClient: SynapClient;

  beforeEach(() => {
    mockClient = {
      sendCommand: vi.fn(),
    } as unknown as SynapClient;
    manager = new BitmapManager(mockClient);
  });

  describe('setbit', () => {
    it('should set bit and return old value', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'test',
        old_value: 0,
      });

      const result = await manager.setbit('test', 5, 1);
      expect(result).toBe(0);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.setbit', {
        key: 'test',
        offset: 5,
        value: 1,
      });
    });

    it('should throw error for invalid value', async () => {
      await expect(manager.setbit('test', 5, 2 as any)).rejects.toThrow(
        'Bitmap value must be 0 or 1'
      );
    });
  });

  describe('getbit', () => {
    it('should get bit value', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'test',
        offset: 5,
        value: 1,
      });

      const result = await manager.getbit('test', 5);
      expect(result).toBe(1);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.getbit', {
        key: 'test',
        offset: 5,
      });
    });

    it('should return 0 for missing bit', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'test',
        offset: 5,
        value: 0,
      });

      const result = await manager.getbit('test', 5);
      expect(result).toBe(0);
    });
  });

  describe('bitcount', () => {
    it('should count all bits when no range specified', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'test',
        count: 5,
      });

      const result = await manager.bitcount('test');
      expect(result).toBe(5);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.bitcount', {
        key: 'test',
      });
    });

    it('should count bits in range', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'test',
        count: 3,
      });

      const result = await manager.bitcount('test', 0, 15);
      expect(result).toBe(3);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.bitcount', {
        key: 'test',
        start: 0,
        end: 15,
      });
    });
  });

  describe('bitpos', () => {
    it('should find position of set bit', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'test',
        position: 7,
      });

      const result = await manager.bitpos('test', 1);
      expect(result).toBe(7);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.bitpos', {
        key: 'test',
        value: 1,
      });
    });

    it('should return null when bit not found', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'test',
        position: null,
      });

      const result = await manager.bitpos('test', 1);
      expect(result).toBeNull();
    });

    it('should throw error for invalid value', async () => {
      await expect(manager.bitpos('test', 2 as any)).rejects.toThrow(
        'Bitmap value must be 0 or 1'
      );
    });

    it('should find position with range', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'test',
        position: 10,
      });

      const result = await manager.bitpos('test', 1, 5, 20);
      expect(result).toBe(10);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.bitpos', {
        key: 'test',
        value: 1,
        start: 5,
        end: 20,
      });
    });
  });

  describe('bitop', () => {
    it('should perform AND operation', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        destination: 'result',
        length: 8,
      });

      const result = await manager.bitop('AND', 'result', ['bitmap1', 'bitmap2']);
      expect(result).toBe(8);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.bitop', {
        destination: 'result',
        operation: 'AND',
        source_keys: ['bitmap1', 'bitmap2'],
      });
    });

    it('should perform OR operation', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        destination: 'result',
        length: 16,
      });

      const result = await manager.bitop('OR', 'result', ['bitmap1', 'bitmap2']);
      expect(result).toBe(16);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.bitop', {
        destination: 'result',
        operation: 'OR',
        source_keys: ['bitmap1', 'bitmap2'],
      });
    });

    it('should perform XOR operation', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        destination: 'result',
        length: 12,
      });

      const result = await manager.bitop('XOR', 'result', ['bitmap1', 'bitmap2']);
      expect(result).toBe(12);
    });

    it('should perform NOT operation with single source', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        destination: 'result',
        length: 8,
      });

      const result = await manager.bitop('NOT', 'result', ['bitmap1']);
      expect(result).toBe(8);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.bitop', {
        destination: 'result',
        operation: 'NOT',
        source_keys: ['bitmap1'],
      });
    });

    it('should throw error for NOT with multiple sources', async () => {
      await expect(
        manager.bitop('NOT', 'result', ['bitmap1', 'bitmap2'])
      ).rejects.toThrow('NOT operation requires exactly one source key');
    });

    it('should throw error for empty sources', async () => {
      await expect(manager.bitop('AND', 'result', [])).rejects.toThrow(
        'BITOP requires at least one source key'
      );
    });
  });

  describe('stats', () => {
    it('should retrieve bitmap statistics', async () => {
      const mockStats = {
        total_bitmaps: 10,
        total_bits: 1000,
        setbit_count: 50,
        getbit_count: 30,
        bitcount_count: 20,
        bitop_count: 5,
        bitpos_count: 15,
        bitfield_count: 0,
      };

      vi.mocked(mockClient.sendCommand).mockResolvedValue(mockStats);

      const result = await manager.stats();
      expect(result).toEqual(mockStats);
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.stats', {});
    });
  });

  describe('options', () => {
    it('should pass clientId in options', async () => {
      vi.mocked(mockClient.sendCommand).mockResolvedValue({
        key: 'test',
        old_value: 0,
      });

      await manager.setbit('test', 5, 1, { clientId: 'tx-123' });
      expect(mockClient.sendCommand).toHaveBeenCalledWith('bitmap.setbit', {
        key: 'test',
        offset: 5,
        value: 1,
        client_id: 'tx-123',
      });
    });
  });
});

