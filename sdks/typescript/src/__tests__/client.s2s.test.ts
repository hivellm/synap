/**
 * Base Client Tests
 */

import { describe, it, expect } from 'vitest';
import { Synap, SynapClient } from '../index';

describe('SynapClient', () => {
  describe('Connection', () => {
    it('should ping the server', async () => {
      const client = new SynapClient({
        url: process.env.SYNAP_URL || 'http://localhost:15500',
      });

      const pong = await client.ping();
      expect(pong).toBe(true);
    });

    it('should get health status', async () => {
      const client = new SynapClient({
        url: process.env.SYNAP_URL || 'http://localhost:15500',
      });

      const health = await client.health();
      expect(health.status).toBe('healthy');
      expect(health.service).toBe('synap');
      expect(health.version).toBeTruthy();
    });
  });

  describe('Main Synap client', () => {
    it('should create client with default options', () => {
      const synap = new Synap();
      expect(synap).toBeInstanceOf(Synap);
      expect(synap.kv).toBeDefined();
      expect(synap.queue).toBeDefined();
    });

    it('should create client with custom URL', () => {
      const synap = new Synap({
        url: 'http://custom-host:9999',
      });
      expect(synap).toBeInstanceOf(Synap);
    });

    it('should ping successfully', async () => {
      const synap = new Synap({
        url: process.env.SYNAP_URL || 'http://localhost:15500',
      });

      const result = await synap.ping();
      expect(result).toBe(true);
    });
  });
});

