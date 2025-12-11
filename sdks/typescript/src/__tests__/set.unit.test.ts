/**
 * Unit tests for Set Manager
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { SetManager } from '../set';
import { SynapClient } from '../client';

describe('SetManager', () => {
  let client: SynapClient;
  let set: SetManager;

  beforeEach(() => {
    client = new SynapClient({ url: 'http://localhost:15500' });
    set = new SetManager(client);
    
    vi.spyOn(client, 'sendCommand');
  });

  it('should add members to set', async () => {
    (client.sendCommand as any).mockResolvedValue({ added: 3 });

    const result = await set.add('tags', 'python', 'redis', 'typescript');

    expect(result).toBe(3);
    expect(client.sendCommand).toHaveBeenCalledWith('set.add', {
      key: 'tags',
      members: ['python', 'redis', 'typescript'],
    });
  });

  it('should remove members from set', async () => {
    (client.sendCommand as any).mockResolvedValue({ removed: 1 });

    const result = await set.rem('tags', 'typescript');

    expect(result).toBe(1);
    expect(client.sendCommand).toHaveBeenCalledWith('set.rem', {
      key: 'tags',
      members: ['typescript'],
    });
  });

  it('should check if member exists', async () => {
    (client.sendCommand as any).mockResolvedValue({ is_member: true });

    const result = await set.isMember('tags', 'python');

    expect(result).toBe(true);
    expect(client.sendCommand).toHaveBeenCalledWith('set.ismember', {
      key: 'tags',
      member: 'python',
    });
  });

  it('should get all members', async () => {
    (client.sendCommand as any).mockResolvedValue({
      members: ['python', 'redis'],
    });

    const result = await set.members('tags');

    expect(result).toEqual(['python', 'redis']);
  });

  it('should get set cardinality', async () => {
    (client.sendCommand as any).mockResolvedValue({ cardinality: 3 });

    const result = await set.card('tags');

    expect(result).toBe(3);
  });

  it('should pop random members', async () => {
    (client.sendCommand as any).mockResolvedValue({ members: ['python'] });

    const result = await set.pop('tags', 1);

    expect(result).toEqual(['python']);
  });

  it('should get random members', async () => {
    (client.sendCommand as any).mockResolvedValue({
      members: ['redis', 'python'],
    });

    const result = await set.randMember('tags', 2);

    expect(result).toEqual(['redis', 'python']);
  });

  it('should move member between sets', async () => {
    (client.sendCommand as any).mockResolvedValue({ moved: true });

    const result = await set.move('tags1', 'tags2', 'python');

    expect(result).toBe(true);
    expect(client.sendCommand).toHaveBeenCalledWith('set.move', {
      source: 'tags1',
      destination: 'tags2',
      member: 'python',
    });
  });

  it('should get intersection of sets', async () => {
    (client.sendCommand as any).mockResolvedValue({ members: ['python'] });

    const result = await set.inter('tags1', 'tags2');

    expect(result).toEqual(['python']);
    expect(client.sendCommand).toHaveBeenCalledWith('set.inter', {
      keys: ['tags1', 'tags2'],
    });
  });

  it('should get union of sets', async () => {
    (client.sendCommand as any).mockResolvedValue({
      members: ['python', 'redis', 'typescript'],
    });

    const result = await set.union('tags1', 'tags2');

    expect(result).toEqual(['python', 'redis', 'typescript']);
  });

  it('should get difference of sets', async () => {
    (client.sendCommand as any).mockResolvedValue({ members: ['redis'] });

    const result = await set.diff('tags1', 'tags2');

    expect(result).toEqual(['redis']);
  });

  it('should store intersection', async () => {
    (client.sendCommand as any).mockResolvedValue({ cardinality: 1 });

    const result = await set.interStore('result', 'tags1', 'tags2');

    expect(result).toBe(1);
  });

  it('should store union', async () => {
    (client.sendCommand as any).mockResolvedValue({ cardinality: 5 });

    const result = await set.unionStore('result', 'tags1', 'tags2');

    expect(result).toBe(5);
  });

  it('should store difference', async () => {
    (client.sendCommand as any).mockResolvedValue({ cardinality: 2 });

    const result = await set.diffStore('result', 'tags1', 'tags2');

    expect(result).toBe(2);
  });
});

