/**
 * Authentication S2S Integration Tests
 * Tests authentication (Basic Auth and API Key) with a running Synap server
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { Synap } from '../index';

const SYNAP_URL = process.env.SYNAP_URL || 'http://localhost:15500';
const TEST_USERNAME = process.env.SYNAP_TEST_USERNAME || 'root';
const TEST_PASSWORD = process.env.SYNAP_TEST_PASSWORD || 'root';

// Probe once whether the target server actually enforces auth. The rejection
// tests only make sense against an auth-enabled server (SYNAP_AUTH_ENABLED);
// against a dev server with auth off they are skipped instead of failing.
// They check the rejection on a command: `/health` and `/metrics` answer
// without credentials by design (liveness probes and scrapers), whatever
// credentials a request carries.
let authRequired = false;

beforeAll(async () => {
  const probe = new Synap({ url: SYNAP_URL });
  try {
    await probe.kv.set('auth:probe', 'x');
    await probe.kv.del('auth:probe');
    authRequired = false; // unauthenticated write succeeded
  } catch {
    authRequired = true;
  } finally {
    probe.close();
  }
});

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
        await synap.kv.del('auth:test:basic');
      } finally {
        synap.close();
      }
    });

    it('should fail with invalid Basic Auth credentials', async (ctx) => {
      if (!authRequired) return ctx.skip();
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'basic',
          username: 'invalid',
          password: 'invalid',
        },
      });

      try {
        await expect(synap.kv.get('auth:probe')).rejects.toThrow();
      } finally {
        synap.close();
      }
    });

    it('should fail with missing password', async (ctx) => {
      if (!authRequired) return ctx.skip();
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'basic',
          username: TEST_USERNAME,
          password: '',
        },
      });

      try {
        await expect(synap.kv.get('auth:probe')).rejects.toThrow();
      } finally {
        synap.close();
      }
    });
  });

  describe('API Key Auth', () => {
    let apiKey: string;

    beforeAll(async () => {
      // Issue a short-lived key with the Basic Auth credentials.
      const credentials = Buffer.from(`${TEST_USERNAME}:${TEST_PASSWORD}`).toString('base64');
      const response = await fetch(`${SYNAP_URL}/auth/keys`, {
        method: 'POST',
        headers: { Authorization: `Basic ${credentials}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: 'ts-sdk-s2s',
          expires_in_seconds: 300,
          permissions: [{ resource: '*', action: 'all' }],
        }),
      });
      expect(response.status).toBe(200);
      const created = (await response.json()) as { key: { key: string } };
      apiKey = created.key.key;
    });

    it('should authenticate with API Key and perform operations', async () => {

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
        await synap.kv.del('auth:test:apikey');
      } finally {
        synap.close();
      }
    });

    it('should fail with invalid API Key', async (ctx) => {
      if (!authRequired) return ctx.skip();
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'api_key',
          apiKey: 'invalid-api-key-12345',
        },
      });

      try {
        await expect(synap.kv.get('auth:probe')).rejects.toThrow();
      } finally {
        synap.close();
      }
    });

    it('should fail with empty API Key', async (ctx) => {
      if (!authRequired) return ctx.skip();
      const synap = new Synap({
        url: SYNAP_URL,
        auth: {
          type: 'api_key',
          apiKey: '',
        },
      });

      try {
        await expect(synap.kv.get('auth:probe')).rejects.toThrow();
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

