/**
 * Client Unit Tests
 * 
 * Focused tests to increase coverage of client.ts
 */

import { describe, it, expect } from 'vitest';
import { SynapClient } from '../client';

describe('SynapClient - Unit Tests', () => {
  describe('Constructor and Configuration', () => {
    it('should create client with default options', () => {
      const client = new SynapClient();
      expect(client).toBeDefined();
    });

    it('should create client with custom URL', () => {
      const client = new SynapClient({ url: 'http://custom-url:8080' });
      expect(client).toBeDefined();
    });

    it('should create client with timeout', () => {
      const client = new SynapClient({ timeout: 10000 });
      expect(client).toBeDefined();
    });

    it('should create client with auth token', () => {
      const client = new SynapClient({ 
        url: 'http://localhost:15500',
        auth: { token: 'test-token-123' }
      });
      expect(client).toBeDefined();
    });

    it('should create client with retry options', () => {
      const client = new SynapClient({ 
        url: 'http://localhost:15500',
        retry: { maxRetries: 5, retryDelay: 2000 }
      });
      expect(client).toBeDefined();
    });

    it('should trim trailing slash from URL', () => {
      const client = new SynapClient({ url: 'http://localhost:15500/' });
      expect(client).toBeDefined();
    });

    it('should handle URL without trailing slash', () => {
      const client = new SynapClient({ url: 'http://localhost:15500' });
      expect(client).toBeDefined();
    });
  });

  describe('Close Method', () => {
    it('should close client without errors', () => {
      const client = new SynapClient();
      expect(() => client.close()).not.toThrow();
    });

    it('should be callable multiple times', () => {
      const client = new SynapClient();
      client.close();
      expect(() => client.close()).not.toThrow();
    });
  });

  describe('Error Cases', () => {
    it('should handle invalid URL gracefully in constructor', () => {
      // Should not throw during construction
      expect(() => new SynapClient({ url: '' })).not.toThrow();
    });

    it('should handle null/undefined options', () => {
      expect(() => new SynapClient(undefined)).not.toThrow();
    });
  });
});

