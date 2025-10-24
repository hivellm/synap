/**
 * Types Unit Tests
 * 
 * Simple tests to cover error classes in types.ts
 */

import { describe, it, expect } from 'vitest';
import { SynapError, NetworkError, ServerError, TimeoutError } from '../types';

describe('Error Classes - Unit Tests', () => {
  describe('SynapError', () => {
    it('should create basic error', () => {
      const error = new SynapError('Test message');
      expect(error.message).toBe('Test message');
      expect(error.name).toBe('SynapError');
    });

    it('should create error with code', () => {
      const error = new SynapError('Test', 'TEST_CODE');
      expect(error.code).toBe('TEST_CODE');
    });

    it('should create error with status code', () => {
      const error = new SynapError('Test', undefined, 404);
      expect(error.statusCode).toBe(404);
    });

    it('should create error with request ID', () => {
      const error = new SynapError('Test', undefined, undefined, 'req-123');
      expect(error.requestId).toBe('req-123');
    });
  });

  describe('NetworkError', () => {
    it('should create network error', () => {
      const error = new NetworkError('Connection failed');
      expect(error.message).toBe('Connection failed');
      expect(error.name).toBe('NetworkError');
      expect(error.code).toBe('NETWORK_ERROR');
    });

    it('should store original error', () => {
      const originalError = new Error('Original');
      const error = new NetworkError('Network failed', originalError);
      expect(error.originalError).toBe(originalError);
    });
  });

  describe('TimeoutError', () => {
    it('should create timeout error', () => {
      const error = new TimeoutError('Timeout', 5000);
      expect(error.message).toBe('Timeout');
      expect(error.name).toBe('TimeoutError');
      expect(error.code).toBe('TIMEOUT_ERROR');
      expect(error.timeoutMs).toBe(5000);
    });
  });

  describe('ServerError', () => {
    it('should create server error', () => {
      const error = new ServerError('Server error');
      expect(error.message).toBe('Server error');
      expect(error.name).toBe('ServerError');
      expect(error.code).toBe('SERVER_ERROR');
    });

    it('should include status code', () => {
      const error = new ServerError('Error', 500);
      expect(error.statusCode).toBe(500);
    });

    it('should include request ID', () => {
      const error = new ServerError('Error', 500, 'req-456');
      expect(error.requestId).toBe('req-456');
    });
  });
});

