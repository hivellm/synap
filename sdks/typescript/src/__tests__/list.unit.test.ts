/**
 * Unit tests for List Manager
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ListManager } from '../list';
import { SynapClient } from '../client';

describe('ListManager', () => {
  let client: SynapClient;
  let list: ListManager;

  beforeEach(() => {
    client = new SynapClient({ url: 'http://localhost:15500' });
    list = new ListManager(client);
    
    vi.spyOn(client, 'sendCommand');
  });

  it('should lpush elements', async () => {
    (client.sendCommand as any).mockResolvedValue({ length: 3 });

    const result = await list.lpush('tasks', 'task1', 'task2', 'task3');

    expect(result).toBe(3);
    expect(client.sendCommand).toHaveBeenCalledWith('list.lpush', {
      key: 'tasks',
      values: ['task1', 'task2', 'task3'],
    });
  });

  it('should rpush elements', async () => {
    (client.sendCommand as any).mockResolvedValue({ length: 2 });

    const result = await list.rpush('tasks', 'task1', 'task2');

    expect(result).toBe(2);
    expect(client.sendCommand).toHaveBeenCalledWith('list.rpush', {
      key: 'tasks',
      values: ['task1', 'task2'],
    });
  });

  it('should lpop elements', async () => {
    (client.sendCommand as any).mockResolvedValue({ values: ['task1'] });

    const result = await list.lpop('tasks');

    expect(result).toEqual(['task1']);
    expect(client.sendCommand).toHaveBeenCalledWith('list.lpop', {
      key: 'tasks',
      count: 1,
    });
  });

  it('should rpop elements', async () => {
    (client.sendCommand as any).mockResolvedValue({ values: ['task3', 'task2'] });

    const result = await list.rpop('tasks', 2);

    expect(result).toEqual(['task3', 'task2']);
    expect(client.sendCommand).toHaveBeenCalledWith('list.rpop', {
      key: 'tasks',
      count: 2,
    });
  });

  it('should get range of elements', async () => {
    (client.sendCommand as any).mockResolvedValue({
      values: ['task1', 'task2', 'task3'],
    });

    const result = await list.range('tasks', 0, -1);

    expect(result).toEqual(['task1', 'task2', 'task3']);
    expect(client.sendCommand).toHaveBeenCalledWith('list.range', {
      key: 'tasks',
      start: 0,
      stop: -1,
    });
  });

  it('should get list length', async () => {
    (client.sendCommand as any).mockResolvedValue({ length: 5 });

    const result = await list.len('tasks');

    expect(result).toBe(5);
  });

  it('should get element at index', async () => {
    (client.sendCommand as any).mockResolvedValue({ value: 'task2' });

    const result = await list.index('tasks', 1);

    expect(result).toBe('task2');
  });

  it('should set element at index', async () => {
    (client.sendCommand as any).mockResolvedValue({ success: true });

    const result = await list.set('tasks', 0, 'new_task');

    expect(result).toBe(true);
  });

  it('should trim list', async () => {
    (client.sendCommand as any).mockResolvedValue({ success: true });

    const result = await list.trim('tasks', 0, 10);

    expect(result).toBe(true);
  });

  it('should remove elements', async () => {
    (client.sendCommand as any).mockResolvedValue({ removed: 2 });

    const result = await list.rem('tasks', 0, 'task1');

    expect(result).toBe(2);
  });

  it('should insert element', async () => {
    (client.sendCommand as any).mockResolvedValue({ length: 5 });

    const result = await list.insert('tasks', 'BEFORE', 'task2', 'new_task');

    expect(result).toBe(5);
  });

  it('should rpoplpush', async () => {
    (client.sendCommand as any).mockResolvedValue({ value: 'task3' });

    const result = await list.rpoplpush('source', 'dest');

    expect(result).toBe('task3');
  });

  it('should find position', async () => {
    (client.sendCommand as any).mockResolvedValue({ position: 2 });

    const result = await list.pos('tasks', 'task3');

    expect(result).toBe(2);
  });

  it('should lpushx', async () => {
    (client.sendCommand as any).mockResolvedValue({ length: 4 });

    const result = await list.lpushx('tasks', 'task0');

    expect(result).toBe(4);
  });

  it('should rpushx', async () => {
    (client.sendCommand as any).mockResolvedValue({ length: 4 });

    const result = await list.rpushx('tasks', 'task4');

    expect(result).toBe(4);
  });
});

