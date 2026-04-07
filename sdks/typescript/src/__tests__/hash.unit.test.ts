/**
 * Unit tests for Hash Manager
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { HashManager } from '../hash';
import { SynapClient } from '../client';

describe('HashManager', () => {
  let client: SynapClient;
  let hash: HashManager;

  beforeEach(() => {
    client = new SynapClient({ url: 'http://localhost:15500' });
    hash = new HashManager(client);
    
    // Mock sendCommand
    vi.spyOn(client, 'sendCommand');
  });

  it('should set hash field', async () => {
    (client.sendCommand as any).mockResolvedValue({ success: true });

    const result = await hash.set('user:1', 'name', 'Alice');

    expect(result).toBe(true);
    expect(client.sendCommand).toHaveBeenCalledWith('hash.set', {
      key: 'user:1',
      field: 'name',
      value: 'Alice',
    });
  });

  it('should get hash field', async () => {
    (client.sendCommand as any).mockResolvedValue({ value: 'Alice' });

    const result = await hash.get('user:1', 'name');

    expect(result).toBe('Alice');
    expect(client.sendCommand).toHaveBeenCalledWith('hash.get', {
      key: 'user:1',
      field: 'name',
    });
  });

  it('should get all fields from hash', async () => {
    (client.sendCommand as any).mockResolvedValue({
      fields: { name: 'Alice', age: '30' },
    });

    const result = await hash.getAll('user:1');

    expect(result).toEqual({ name: 'Alice', age: '30' });
    expect(client.sendCommand).toHaveBeenCalledWith('hash.getall', {
      key: 'user:1',
    });
  });

  it('should delete hash field', async () => {
    (client.sendCommand as any).mockResolvedValue({ deleted: 1 });

    const result = await hash.del('user:1', 'name');

    expect(result).toBe(1);
    expect(client.sendCommand).toHaveBeenCalledWith('hash.del', {
      key: 'user:1',
      field: 'name',
    });
  });

  it('should check if hash field exists', async () => {
    (client.sendCommand as any).mockResolvedValue({ exists: true });

    const result = await hash.exists('user:1', 'name');

    expect(result).toBe(true);
    expect(client.sendCommand).toHaveBeenCalledWith('hash.exists', {
      key: 'user:1',
      field: 'name',
    });
  });

  it('should get hash keys', async () => {
    (client.sendCommand as any).mockResolvedValue({
      fields: ['name', 'age'],
    });

    const result = await hash.keys('user:1');

    expect(result).toEqual(['name', 'age']);
  });

  it('should get hash values', async () => {
    (client.sendCommand as any).mockResolvedValue({
      values: ['Alice', '30'],
    });

    const result = await hash.values('user:1');

    expect(result).toEqual(['Alice', '30']);
  });

  it('should get hash length', async () => {
    (client.sendCommand as any).mockResolvedValue({ length: 2 });

    const result = await hash.len('user:1');

    expect(result).toBe(2);
  });

  it('should set multiple hash fields', async () => {
    (client.sendCommand as any).mockResolvedValue({ success: true });

    const result = await hash.mset('user:1', {
      name: 'Alice',
      age: 30,
    });

    expect(result).toBe(true);
    expect(client.sendCommand).toHaveBeenCalledWith('hash.mset', {
      key: 'user:1',
      fields: { name: 'Alice', age: '30' },
    });
  });

  it('should get multiple hash fields', async () => {
    (client.sendCommand as any).mockResolvedValue({
      values: { name: 'Alice', age: '30' },
    });

    const result = await hash.mget('user:1', ['name', 'age']);

    expect(result).toEqual({ name: 'Alice', age: '30' });
  });

  it('should increment hash field by integer', async () => {
    (client.sendCommand as any).mockResolvedValue({ value: 5 });

    const result = await hash.incrBy('counters', 'visits', 1);

    expect(result).toBe(5);
    expect(client.sendCommand).toHaveBeenCalledWith('hash.incrby', {
      key: 'counters',
      field: 'visits',
      increment: 1,
    });
  });

  it('should increment hash field by float', async () => {
    (client.sendCommand as any).mockResolvedValue({ value: 3.14 });

    const result = await hash.incrByFloat('metrics', 'score', 0.5);

    expect(result).toBe(3.14);
    expect(client.sendCommand).toHaveBeenCalledWith('hash.incrbyfloat', {
      key: 'metrics',
      field: 'score',
      increment: 0.5,
    });
  });

  it('should set hash field if not exists', async () => {
    (client.sendCommand as any).mockResolvedValue({ created: true });

    const result = await hash.setNX('user:1', 'email', 'alice@example.com');

    expect(result).toBe(true);
    expect(client.sendCommand).toHaveBeenCalledWith('hash.setnx', {
      key: 'user:1',
      field: 'email',
      value: 'alice@example.com',
    });
  });
});

