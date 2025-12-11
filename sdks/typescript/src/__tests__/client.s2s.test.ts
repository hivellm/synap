/**
 * Client S2S Integration Tests
 * Tests client configuration, error handling, and network scenarios
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { Synap } from '../index';

describe('Synap Client (S2S Integration)', () => {
  let synap: Synap;

  beforeAll(() => {
    synap = new Synap({
      url: process.env.SYNAP_URL || 'http://localhost:15500',
      timeout: 5000,
    });
  });

  afterAll(() => {
    synap.close();
  });

  describe('Client Configuration', () => {
    it('should create client with default configuration', () => {
      const defaultClient = new Synap();
      expect(defaultClient).toBeTruthy();
      defaultClient.close();
    });

    it('should create client with custom URL', () => {
      const customClient = new Synap({
        url: 'http://custom-host:9999',
      });
      expect(customClient).toBeTruthy();
      customClient.close();
    });

    it('should create client with timeout configuration', () => {
      const timeoutClient = new Synap({
        timeout: 10000,
      });
      expect(timeoutClient).toBeTruthy();
      timeoutClient.close();
    });

    it('should create client with authentication config', () => {
      const authClient = new Synap({
        auth: {
          type: 'basic',
          username: 'testuser',
          password: 'testpass',
        },
      });
      expect(authClient).toBeTruthy();
      authClient.close();
    });

    it('should create client with api_key auth', () => {
      const bearerClient = new Synap({
        auth: {
          type: 'api_key',
          apiKey: 'test-api-key',
        },
      });
      expect(bearerClient).toBeTruthy();
      bearerClient.close();
    });

    it('should create client with all options', () => {
      const fullClient = new Synap({
        url: 'http://localhost:15500',
        timeout: 15000,
        auth: {
          type: 'basic',
          username: 'user',
          password: 'pass',
        },
      });
      expect(fullClient).toBeTruthy();
      fullClient.close();
    });
  });

  describe('Module Access', () => {
    it('should access KV module', () => {
      const kv = synap.kv;
      expect(kv).toBeTruthy();
      expect(typeof kv.set).toBe('function');
      expect(typeof kv.get).toBe('function');
      expect(typeof kv.del).toBe('function');
    });

    it('should access Queue module', () => {
      const queue = synap.queue;
      expect(queue).toBeTruthy();
      expect(typeof queue.publish).toBe('function');
      expect(typeof queue.consume).toBe('function');
    });

    it('should access Stream module', () => {
      const stream = synap.stream;
      expect(stream).toBeTruthy();
      expect(typeof stream.publish).toBe('function');
      expect(typeof stream.consume).toBe('function');
    });

    it('should access PubSub module', () => {
      const pubsub = synap.pubsub;
      expect(pubsub).toBeTruthy();
      expect(typeof pubsub.publish).toBe('function');
      expect(typeof pubsub.subscribe).toBe('function');
    });

    it('should provide same module instances', () => {
      const kv1 = synap.kv;
      const kv2 = synap.kv;
      expect(kv1).toBe(kv2);

      const queue1 = synap.queue;
      const queue2 = synap.queue;
      expect(queue1).toBe(queue2);
    });
  });

  describe('Client Lifecycle', () => {
    it('should close client gracefully', () => {
      const tempClient = new Synap();
      expect(() => tempClient.close()).not.toThrow();
    });

    it('should handle multiple close calls', () => {
      const tempClient = new Synap();
      expect(() => {
        tempClient.close();
        tempClient.close();
      }).not.toThrow();
    });

    it('should access all modules before close', () => {
      const tempClient = new Synap();
      const kv = tempClient.kv;
      const queue = tempClient.queue;
      const stream = tempClient.stream;
      const pubsub = tempClient.pubsub;

      expect(kv).toBeTruthy();
      expect(queue).toBeTruthy();
      expect(stream).toBeTruthy();
      expect(pubsub).toBeTruthy();

      tempClient.close();
    });
  });

  describe('Network Edge Cases', () => {
    it('should handle client creation with empty config', () => {
      const emptyClient = new Synap({});
      expect(emptyClient).toBeTruthy();
      emptyClient.close();
    });

    it('should handle URL normalization', () => {
      const client1 = new Synap({ url: 'http://localhost:15500/' });
      const client2 = new Synap({ url: 'http://localhost:15500' });

      expect(client1).toBeTruthy();
      expect(client2).toBeTruthy();

      client1.close();
      client2.close();
    });

    it('should handle HTTPS URLs', () => {
      const httpsClient = new Synap({
        url: 'https://secure.example.com:15500',
      });
      expect(httpsClient).toBeTruthy();
      httpsClient.close();
    });

    it('should handle various port numbers', () => {
      const ports = [8080, 15500, 3000, 65535];
      ports.forEach(port => {
        const portClient = new Synap({
          url: `http://localhost:${port}`,
        });
        expect(portClient).toBeTruthy();
        portClient.close();
      });
    });

    it('should handle timeout configuration variations', () => {
      const configs = [
        { timeout: 0 },
        { timeout: 100 },
        { timeout: 30000 },
        { timeout: 60000 },
      ];

      configs.forEach(config => {
        const timeoutClient = new Synap(config);
        expect(timeoutClient).toBeTruthy();
        timeoutClient.close();
      });
    });
  });

  describe('Authentication Scenarios', () => {
    it('should handle basic auth with special characters', () => {
      const authClient = new Synap({
        auth: {
          type: 'basic',
          username: 'user@example.com',
          password: 'p@ssw0rd!#$',
        },
      });
      expect(authClient).toBeTruthy();
      authClient.close();
    });

    it('should handle api_key with various formats', () => {
      const tokens = [
        'simple-token',
        'token-with-dashes-and-numbers-123',
        'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9',
      ];

      tokens.forEach(token => {
        const apiKeyClient = new Synap({
          auth: {
            type: 'api_key',
            apiKey: token,
          },
        });
        expect(apiKeyClient).toBeTruthy();
        apiKeyClient.close();
      });
    });
  });

  describe('Real Server Integration', () => {
    it('should connect to real server', async () => {
      const testKey = `client-test-${Date.now()}`;
      const testValue = 'integration-test-value';

      // Set a value
      await synap.kv.set(testKey, testValue);

      // Get the value back
      const retrieved = await synap.kv.get(testKey);
      expect(retrieved).toBe(testValue);

      // Clean up
      await synap.kv.del(testKey);
    });

    it('should access all modules on real server', async () => {
      // Smoke test for each module
      const kvKey = `module-test-${Date.now()}`;
      await synap.kv.set(kvKey, 'test');
      expect(await synap.kv.get(kvKey)).toBe('test');
      await synap.kv.del(kvKey);

      // Verify modules are accessible
      expect(synap.kv).toBeTruthy();
      expect(synap.queue).toBeTruthy();
      expect(synap.stream).toBeTruthy();
      expect(synap.pubsub).toBeTruthy();
    });
  });

  describe('Server Health', () => {
    it('should check server health', async () => {
      const health = await synap.health();
      expect(health).toBeDefined();
      expect(health.status).toBeTruthy();
    });

    it('should ping server', async () => {
      const pong = await synap.ping();
      expect(typeof pong).toBe('boolean');
    });

    it('should get underlying client', () => {
      const client = synap.getClient();
      expect(client).toBeTruthy();
      expect(typeof client.sendCommand).toBe('function');
    });
  });
});

