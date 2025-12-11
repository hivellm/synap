import { describe, it, expect, beforeEach, vi } from 'vitest';
import { HyperLogLogManager } from '../hyperloglog';
import { createMockClient } from './__mocks__/client.mock';
import type { SynapClient } from '../client';

describe('HyperLogLogManager', () => {
  let mockClient: SynapClient;
  let hll: HyperLogLogManager;

  beforeEach(() => {
    mockClient = createMockClient();
    hll = new HyperLogLogManager(mockClient);
    vi.clearAllMocks();
  });

  it('should encode elements and call PFADD', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ added: 2 });

    const added = await hll.pfadd('unique-users', ['user:1', 'user:2']);

    expect(added).toBe(2);
    expect(mockClient.sendCommand).toHaveBeenCalledWith('hyperloglog.pfadd', {
      key: 'unique-users',
      elements: [
        Array.from(new TextEncoder().encode('user:1')),
        Array.from(new TextEncoder().encode('user:2')),
      ],
    });
  });

  it('should include client_id when provided', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ count: 10 });

    await hll.pfcount('unique-users', { clientId: 'client-123' });

    expect(mockClient.sendCommand).toHaveBeenCalledWith('hyperloglog.pfcount', {
      key: 'unique-users',
      client_id: 'client-123',
    });
  });

  it('should merge HyperLogLogs', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ count: 50 });

    const count = await hll.pfmerge('dest', ['hll:1', 'hll:2']);

    expect(count).toBe(50);
    expect(mockClient.sendCommand).toHaveBeenCalledWith('hyperloglog.pfmerge', {
      destination: 'dest',
      sources: ['hll:1', 'hll:2'],
    });
  });

  it('should return stats', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({
      total_hlls: 2,
      pfadd_count: 5,
      pfcount_count: 3,
      pfmerge_count: 1,
      total_cardinality: 123,
    });

    const stats = await hll.stats();

    expect(stats).toEqual({
      total_hlls: 2,
      pfadd_count: 5,
      pfcount_count: 3,
      pfmerge_count: 1,
      total_cardinality: 123,
    });
    expect(mockClient.sendCommand).toHaveBeenCalledWith('hyperloglog.stats', {});
  });
});
