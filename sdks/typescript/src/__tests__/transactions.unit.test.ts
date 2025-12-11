import { describe, it, expect, beforeEach, vi } from 'vitest';
import { TransactionManager } from '../transactions';
import { createMockClient } from './__mocks__/client.mock';
import type { SynapClient } from '../client';

describe('TransactionManager', () => {
  let mockClient: SynapClient;
  let transaction: TransactionManager;

  beforeEach(() => {
    mockClient = createMockClient();
    transaction = new TransactionManager(mockClient);
    vi.clearAllMocks();
  });

  it('should start a transaction (MULTI)', async () => {
    const response = await transaction.multi();

    expect(response).toEqual({ success: true, message: 'Transaction started' });
    expect(mockClient.sendCommand).toHaveBeenCalledWith('transaction.multi', {});
  });

  it('should discard a transaction (DISCARD)', async () => {
    const response = await transaction.discard({ clientId: 'client-1' });

    expect(response.message).toBe('Transaction discarded');
    expect(mockClient.sendCommand).toHaveBeenCalledWith('transaction.discard', {
      client_id: 'client-1',
    });
  });

  it('should watch keys and unwatch', async () => {
    const watchResponse = await transaction.watch({ clientId: 'client-1', keys: ['key1', 'key2'] });
    expect(watchResponse.success).toBe(true);
    expect(mockClient.sendCommand).toHaveBeenNthCalledWith(1, 'transaction.watch', {
      client_id: 'client-1',
      keys: ['key1', 'key2'],
    });

    const unwatchResponse = await transaction.unwatch({ clientId: 'client-1' });
    expect(unwatchResponse.success).toBe(true);
    expect(mockClient.sendCommand).toHaveBeenNthCalledWith(2, 'transaction.unwatch', {
      client_id: 'client-1',
    });
  });

  it('should throw when watching without keys', async () => {
    await expect(transaction.watch({ clientId: 'client-1', keys: [] })).rejects.toThrow(
      'Transaction watch requires at least one key'
    );
  });

  it('should parse EXEC success responses', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ results: [1, 'OK'] });

    const result = await transaction.exec({ clientId: 'client-1' });

    expect(result).toEqual({ success: true, results: [1, 'OK'] });
    expect(mockClient.sendCommand).toHaveBeenCalledWith('transaction.exec', {
      client_id: 'client-1',
    });
  });

  it('should parse EXEC aborted responses', async () => {
    vi.mocked(mockClient.sendCommand).mockResolvedValueOnce({ aborted: true, message: 'changed' });

    const result = await transaction.exec({ clientId: 'client-1' });

    expect(result).toEqual({ success: false, aborted: true, message: 'changed' });
  });

  it('should provide a scoped client that injects client_id automatically', async () => {
    const scope = transaction.scope('scoped-client');

    await scope.kv.set('test', 'value');

    expect(mockClient.sendCommand).toHaveBeenCalledWith('kv.set', {
      key: 'test',
      value: 'value',
      client_id: 'scoped-client',
    });
  });
});
