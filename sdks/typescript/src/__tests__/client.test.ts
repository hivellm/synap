/**
 * Client Unit Tests (Mock)
 * Basic client functionality tests
 */

import { describe, it, expect } from 'vitest';
import { SynapClient } from '../client';

describe('SynapClient (Unit Tests)', () => {
  describe('Configuration', () => {
    it('should create client with default config', () => {
      const client = new SynapClient();
      
      expect(client).toBeTruthy();
      expect(client).toBeInstanceOf(SynapClient);
    });

    it('should create client with custom URL', () => {
      const client = new SynapClient({ 
        url: 'http://custom-server:8080' 
      });
      
      expect(client).toBeTruthy();
    });

    it('should create client with timeout', () => {
      const client = new SynapClient({ 
        timeout: 5000 
      });
      
      expect(client).toBeTruthy();
    });

    it('should create client with auth', () => {
      const client = new SynapClient({
        auth: {
          type: 'basic',
          username: 'admin',
          password: 'secret',
        }
      });
      
      expect(client).toBeTruthy();
    });

    it('should close client', () => {
      const client = new SynapClient();
      
      expect(() => client.close()).not.toThrow();
    });
  });
});

