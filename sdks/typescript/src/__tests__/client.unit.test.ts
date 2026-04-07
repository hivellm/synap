/**
 * Client Unit Tests (Mock)
 * Comprehensive unit tests for SynapClient using mocks
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { SynapClient } from '../client';
import { NetworkError, ServerError, TimeoutError } from '../types';

// Mock fetch globally
const mockFetch = vi.fn();
global.fetch = mockFetch as any;

describe('SynapClient (Unit Tests)', () => {
  beforeEach(() => {
    mockFetch.mockClear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('Constructor & Configuration', () => {
    it('should create client with default options', () => {
      const client = new SynapClient();
      expect(client).toBeTruthy();
    });

    it('should create client with custom URL', () => {
      const client = new SynapClient({ url: 'http://custom:9999' });
      expect(client).toBeTruthy();
    });

    it('should create client with timeout', () => {
      const client = new SynapClient({ timeout: 5000 });
      expect(client).toBeTruthy();
    });

    it('should create client with debug enabled', () => {
      const client = new SynapClient({ debug: true });
      expect(client).toBeTruthy();
    });

    it('should create client with basic auth', () => {
      const client = new SynapClient({
        auth: {
          type: 'basic',
          username: 'user',
          password: 'pass',
        },
      });
      expect(client).toBeTruthy();
    });

    it('should create client with api_key auth', () => {
      const client = new SynapClient({
        auth: {
          type: 'api_key',
          apiKey: 'secret-key',
        },
      });
      expect(client).toBeTruthy();
    });
  });

  describe('sendCommand - Success Cases', () => {
    it('should send command successfully', async () => {
      const client = new SynapClient();
      
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: { result: 'data' },
          request_id: 'test-id',
        }),
      });

      const result = await client.sendCommand('test.command', { key: 'value' });
      expect(result).toEqual({ result: 'data' });
      expect(mockFetch).toHaveBeenCalledTimes(1);
    });

    it('should include basic auth header', async () => {
      const client = new SynapClient({
        auth: {
          type: 'basic',
          username: 'testuser',
          password: 'testpass',
        },
      });

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('test.command');
      
      const fetchCall = mockFetch.mock.calls[0];
      const headers = fetchCall[1].headers;
      expect(headers['Authorization']).toMatch(/^Basic /);
    });

    it('should include bearer token header', async () => {
      const client = new SynapClient({
        auth: {
          type: 'api_key',
          apiKey: 'my-secret-key',
        },
      });

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('test.command');
      
      const fetchCall = mockFetch.mock.calls[0];
      const headers = fetchCall[1].headers;
      expect(headers['Authorization']).toBe('Bearer my-secret-key');
    });

    it('should log debug info when debug is enabled', async () => {
      const consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
      const client = new SynapClient({ debug: true });

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('test.command', { test: 'data' });
      
      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringContaining('[Synap] Request:'),
        expect.any(String)
      );
      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringContaining('[Synap] Response:'),
        expect.any(String)
      );

      consoleSpy.mockRestore();
    });

    it('should not log when debug is disabled', async () => {
      const consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
      const client = new SynapClient({ debug: false });

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('test.command');
      expect(consoleSpy).not.toHaveBeenCalled();

      consoleSpy.mockRestore();
    });
  });

  describe('sendCommand - Error Cases', () => {
    it('should throw ServerError on HTTP error', async () => {
      const client = new SynapClient();

      mockFetch.mockResolvedValue({
        ok: false,
        status: 500,
        statusText: 'Internal Server Error',
      });

      await expect(client.sendCommand('test.command')).rejects.toThrow(ServerError);
      await expect(client.sendCommand('test.command')).rejects.toThrow('HTTP 500');
    });

    it('should throw ServerError on success=false in response', async () => {
      const client = new SynapClient();

      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => ({
          success: false,
          error: 'Custom error message',
          request_id: 'test-id',
        }),
      });

      await expect(client.sendCommand('test.command')).rejects.toThrow(ServerError);
      await expect(client.sendCommand('test.command')).rejects.toThrow('Custom error message');
    });

    it('should throw ServerError with default message when error is undefined', async () => {
      const client = new SynapClient();

      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => ({
          success: false,
          request_id: 'test-id',
        }),
      });

      await expect(client.sendCommand('test.command')).rejects.toThrow('Unknown server error');
    });

    it('should throw TimeoutError on abort', async () => {
      const client = new SynapClient({ timeout: 100 });

      mockFetch.mockImplementation(() => 
        new Promise((_, reject) => {
          setTimeout(() => {
            const error = new Error('Aborted');
            error.name = 'AbortError';
            reject(error);
          }, 200);
        })
      );

      await expect(client.sendCommand('test.command')).rejects.toThrow(TimeoutError);
    });

    it('should throw NetworkError on fetch failure', async () => {
      const client = new SynapClient();

      mockFetch.mockRejectedValue(new Error('Network failure'));

      await expect(client.sendCommand('test.command')).rejects.toThrow(NetworkError);
    });

    it('should throw NetworkError on unknown error', async () => {
      const client = new SynapClient();

      mockFetch.mockRejectedValue('unknown error string');

      await expect(client.sendCommand('test.command')).rejects.toThrow(NetworkError);
    });

    it('should rethrow ServerError as-is', async () => {
      const client = new SynapClient();
      const serverError = new ServerError('Test error', 500, 'req-123');

      mockFetch.mockRejectedValue(serverError);

      await expect(client.sendCommand('test.command')).rejects.toThrow(ServerError);
    });
  });

  describe('ping()', () => {
    it('should return true on successful ping', async () => {
      const client = new SynapClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
      });

      const result = await client.ping();
      expect(result).toBe(true);
    });

    it('should return false on failed ping', async () => {
      const client = new SynapClient();

      mockFetch.mockResolvedValueOnce({
        ok: false,
      });

      const result = await client.ping();
      expect(result).toBe(false);
    });

    it('should return false on network error', async () => {
      const client = new SynapClient();

      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      const result = await client.ping();
      expect(result).toBe(false);
    });

    it('should use correct endpoint', async () => {
      const client = new SynapClient({ url: 'http://test:8080' });

      mockFetch.mockResolvedValueOnce({
        ok: true,
      });

      await client.ping();
      
      expect(mockFetch).toHaveBeenCalledWith(
        'http://test:8080/health',
        expect.any(Object)
      );
    });
  });

  describe('health()', () => {
    it('should return health status', async () => {
      const client = new SynapClient();
      const healthData = {
        status: 'healthy',
        service: 'synap',
        version: '1.0.0',
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => healthData,
      });

      const result = await client.health();
      expect(result).toEqual(healthData);
    });

    it('should throw NetworkError on failed health check', async () => {
      const client = new SynapClient();

      mockFetch.mockResolvedValue({
        ok: false,
        status: 500,
        statusText: 'Server Error',
      });

      await expect(client.health()).rejects.toThrow(NetworkError);
    });

    it('should throw on network error', async () => {
      const client = new SynapClient();

      mockFetch.mockRejectedValueOnce(new Error('Connection refused'));

      await expect(client.health()).rejects.toThrow();
    });

    it('should use correct endpoint', async () => {
      const client = new SynapClient({ url: 'http://custom:9999' });

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ status: 'healthy', service: 'synap', version: '1.0.0' }),
      });

      await client.health();
      
      expect(mockFetch).toHaveBeenCalledWith(
        'http://custom:9999/health',
        expect.any(Object)
      );
    });
  });

  describe('close()', () => {
    it('should not throw on close', () => {
      const client = new SynapClient();
      expect(() => client.close()).not.toThrow();
    });

    it('should be callable multiple times', () => {
      const client = new SynapClient();
      expect(() => {
        client.close();
        client.close();
        client.close();
      }).not.toThrow();
    });
  });

  describe('Authentication Edge Cases', () => {
    it('should not add auth header if auth is undefined', async () => {
      const client = new SynapClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('test.command');
      
      const fetchCall = mockFetch.mock.calls[0];
      const headers = fetchCall[1].headers;
      expect(headers['Authorization']).toBeUndefined();
    });

    it('should not add auth header if basic auth missing username', async () => {
      const client = new SynapClient({
        auth: {
          type: 'basic',
          password: 'pass',
        } as any,
      });

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('test.command');
      
      const fetchCall = mockFetch.mock.calls[0];
      const headers = fetchCall[1].headers;
      expect(headers['Authorization']).toBeUndefined();
    });

    it('should not add auth header if api_key missing apiKey', async () => {
      const client = new SynapClient({
        auth: {
          type: 'api_key',
        } as any,
      });

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('test.command');
      
      const fetchCall = mockFetch.mock.calls[0];
      const headers = fetchCall[1].headers;
      expect(headers['Authorization']).toBeUndefined();
    });
  });

  describe('Request Formatting', () => {
    it('should include all required headers', async () => {
      const client = new SynapClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('test.command');
      
      const fetchCall = mockFetch.mock.calls[0];
      const headers = fetchCall[1].headers;
      
      expect(headers['Content-Type']).toBe('application/json');
      expect(headers['Accept']).toBe('application/json');
      expect(headers['Accept-Encoding']).toBe('gzip');
    });

    it('should use POST method', async () => {
      const client = new SynapClient();

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('test.command');
      
      const fetchCall = mockFetch.mock.calls[0];
      expect(fetchCall[1].method).toBe('POST');
    });

    it('should send correct command endpoint', async () => {
      const client = new SynapClient({ url: 'http://localhost:15500' });

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('kv.set');
      
      expect(mockFetch).toHaveBeenCalledWith(
        'http://localhost:15500/api/v1/command',
        expect.any(Object)
      );
    });

    it('should include payload in request body', async () => {
      const client = new SynapClient();
      const payload = { key: 'test-key', value: 'test-value' };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          success: true,
          payload: {},
          request_id: 'test-id',
        }),
      });

      await client.sendCommand('kv.set', payload);
      
      const fetchCall = mockFetch.mock.calls[0];
      const body = JSON.parse(fetchCall[1].body);
      
      expect(body.command).toBe('kv.set');
      expect(body.payload).toEqual(payload);
      expect(body.request_id).toBeTruthy();
    });
  });
});
