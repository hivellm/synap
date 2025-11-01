import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ScriptManager } from '../scripting';
import { createMockClient } from './__mocks__/client.mock';
import type { SynapClient } from '../client';

describe('ScriptManager', () => {
  let mockClient: SynapClient;
  let script: ScriptManager;

  beforeEach(() => {
    mockClient = createMockClient();
    script = new ScriptManager(mockClient);
    vi.clearAllMocks();
  });

  it('should execute EVAL with keys, args, and timeout', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ result: 'OK', sha1: 'abc123' });

    const response = await script.eval('return ARGV[1]', {
      keys: ['key1'],
      args: ['value'],
      timeoutMs: 5000,
    });

    expect(mockClient.sendCommand).toHaveBeenCalledWith('script.eval', {
      script: 'return ARGV[1]',
      keys: ['key1'],
      args: ['value'],
      timeout_ms: 5000,
    });
    expect(response).toEqual({ result: 'OK', sha1: 'abc123' });
  });

  it('should execute EVALSHA', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ result: 42, sha1: 'abc123' });

    const response = await script.evalsha('abc123', {
      keys: ['key'],
      args: [1, 2, 3],
    });

    expect(mockClient.sendCommand).toHaveBeenCalledWith('script.evalsha', {
      sha1: 'abc123',
      keys: ['key'],
      args: [1, 2, 3],
    });
    expect(response).toEqual({ result: 42, sha1: 'abc123' });
  });

  it('should load scripts and return SHA1', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ sha1: 'new-sha' });

    const sha1 = await script.load('return 1');

    expect(mockClient.sendCommand).toHaveBeenCalledWith('script.load', { script: 'return 1' });
    expect(sha1).toBe('new-sha');
  });

  it('should check script existence', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ exists: [true, false] });

    const result = await script.exists(['sha1', 'sha2']);

    expect(mockClient.sendCommand).toHaveBeenCalledWith('script.exists', {
      hashes: ['sha1', 'sha2'],
    });
    expect(result).toEqual([true, false]);
  });

  it('should flush and kill scripts', async () => {
    vi.mocked(mockClient.sendCommand)
      .mockResolvedValueOnce({ cleared: 2 })
      .mockResolvedValueOnce({ terminated: true });

    const cleared = await script.flush();
    const terminated = await script.kill();

    expect(cleared).toBe(2);
    expect(terminated).toBe(true);
    expect(mockClient.sendCommand).toHaveBeenNthCalledWith(1, 'script.flush', {});
    expect(mockClient.sendCommand).toHaveBeenNthCalledWith(2, 'script.kill', {});
  });
});
