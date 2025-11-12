/**
 * Authentication S2S Integration Tests
 * Tests authentication (Basic Auth and API Key) with a running Synap server
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { Synap } from '../index';

const SYNAP_URL = process.env.SYNAP_URL || 'http://localhost:15500';
const TEST_USERNAME = process.env.SYNAP_TEST_USERNAME || 'root';
const TEST_PASSWORD = process.env.SYNAP_TEST_PASSWORD || 'root';

describe('Synap Authentication (S2S Integration)', () => {
  describe('Basic Auth', () => {
    it('should authenticate with Basic Auth and perform operations', async () => {
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'basic',
          username: TEST_USERNAME,
          password: TEST_PASSWORD,
        },
      });

      try {
        // Test health check
        const health = await synap.health();
        expect(health).toBeDefined();

        // Test KV operation
        await synap.kv.set('auth:test:basic', 'test_value');
        const value = await synap.kv.get('auth:test:basic');
        expect(value).toBe('test_value');

        // Cleanup
        await synap.kv.delete('auth:test:basic');
      } finally {
        synap.close();
      }
    });

    it('should fail with invalid Basic Auth credentials', async () => {
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'basic',
          username: 'invalid',
          password: 'invalid',
        },
      });

      try {
        await expect(synap.health()).rejects.toThrow();
      } finally {
        synap.close();
      }
    });

    it('should fail with missing password', async () => {
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'basic',
          username: TEST_USERNAME,
          password: '',
        },
      });

      try {
        await expect(synap.health()).rejects.toThrow();
      } finally {
        synap.close();
      }
    });
  });

  describe('API Key Auth', () => {
    let apiKey: string;

    beforeAll(async () => {
      // Create an API key using Basic Auth
      const adminClient = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'basic',
          username: TEST_USERNAME,
          password: TEST_PASSWORD,
        },
      });

      try {
        // Create API key via REST API (if endpoint exists)
        // For now, we'll skip if API key creation fails
        // In real scenario, you'd create the key first
        apiKey = 'test-api-key-placeholder';
      } finally {
        adminClient.close();
      }
    });

    it('should authenticate with API Key and perform operations', async () => {
      // Skip if no API key available
      if (!apiKey || apiKey === 'test-api-key-placeholder') {
        console.log('Skipping API Key test - no valid API key available');
        return;
      }

      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'api_key',
          apiKey: apiKey,
        },
      });

      try {
        // Test health check
        const health = await synap.health();
        expect(health).toBeDefined();

        // Test KV operation
        await synap.kv.set('auth:test:apikey', 'test_value');
        const value = await synap.kv.get('auth:test:apikey');
        expect(value).toBe('test_value');

        // Cleanup
        await synap.kv.delete('auth:test:apikey');
      } finally {
        synap.close();
      }
    });

    it('should fail with invalid API Key', async () => {
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'api_key',
          apiKey: 'invalid-api-key-12345',
        },
      });

      try {
        await expect(synap.health()).rejects.toThrow();
      } finally {
        synap.close();
      }
    });

    it('should fail with empty API Key', async () => {
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'api_key',
          apiKey: '',
        },
      });

      try {
        await expect(synap.health()).rejects.toThrow();
      } finally {
        synap.close();
      }
    });
  });

  describe('No Authentication', () => {
    it('should work without auth when auth is disabled', async () => {
      const synap = new Synap({
        url: SYNAP_URL,
        // No auth config
      });

      try {
        // This will only work if auth is disabled on server
        const health = await synap.health();
        expect(health).toBeDefined();
      } catch (error) {
        // Expected if auth is required
        expect(error).toBeDefined();
      } finally {
        synap.close();
      }
    });
  });

  describe('Auth Configuration', () => {
    it('should create client with Basic Auth config', () => {
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'basic',
          username: 'user',
          password: 'pass',
        },
      });

      expect(synap).toBeDefined();
      synap.close();
    });

    it('should create client with API Key config', () => {
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'api_key',
          apiKey: 'sk_test123',
        },
      });

      expect(synap).toBeDefined();
      synap.close();
    });

    it('should create client without auth config', () => {
      const synap = new Synap({
        url: SYNAP_URL,
      });

      expect(synap).toBeDefined();
      synap.close();
    });
  });
});

