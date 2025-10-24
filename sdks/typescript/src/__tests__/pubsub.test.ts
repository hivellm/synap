/**
 * Pub/Sub Unit Tests
 * 
 * Mock-based tests for pub/sub functionality
 */

import { describe, it, expect, vi } from 'vitest';
import { PubSubManager } from '../pubsub';
import { SynapClient } from '../client';

describe('PubSubManager - Unit Tests', () => {
  describe('publish()', () => {
    it('should call client with correct payload structure', async () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true, subscribers_matched: 2 })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const topic = 'test.topic';
      const data = { event: 'test' };

      await pubsub.publish(topic, data);

      // ✅ CRITICAL: Verify it sends "payload" field, not "data"
      expect(mockClient.sendCommand).toHaveBeenCalledWith(
        'pubsub.publish',
        expect.objectContaining({
          topic: 'test.topic',
          payload: { event: 'test' }  // Must be "payload", not "data"
        })
      );
    });

    it('should return boolean result', async () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const result = await pubsub.publish('topic', { data: 'test' });

      expect(typeof result).toBe('boolean');
    });

    it('should include priority when provided', async () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      await pubsub.publish('topic', { data: 'test' }, { priority: 5 });

      expect(mockClient.sendCommand).toHaveBeenCalledWith(
        'pubsub.publish',
        expect.objectContaining({
          topic: 'topic',
          payload: { data: 'test' },
          priority: 5
        })
      );
    });

    it('should include headers when provided', async () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const headers = { 'X-Custom': 'value' };
      
      await pubsub.publish('topic', { data: 'test' }, { headers });

      expect(mockClient.sendCommand).toHaveBeenCalledWith(
        'pubsub.publish',
        expect.objectContaining({
          headers: { 'X-Custom': 'value' }
        })
      );
    });
  });

  describe('publishMessage()', () => {
    it('should call publish with correct parameters', async () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const spy = vi.spyOn(pubsub, 'publish');

      await pubsub.publishMessage('topic', { typed: 'data' });

      expect(spy).toHaveBeenCalledWith('topic', { typed: 'data' }, undefined);
    });
  });

  describe('Reactive Methods', () => {
    it('should create observable subscription', () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const observable = pubsub.subscribe({ topics: ['test.*'], subscriberId: 'test-sub' });

      expect(observable).toBeDefined();
      expect(observable.subscribe).toBeDefined();
    });

    it('should call subscribe command on server', async () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      pubsub.subscribe({ topics: ['test.*'], subscriberId: 'test-sub' });

      // Wait for async setup
      await new Promise(resolve => setTimeout(resolve, 10));

      expect(mockClient.sendCommand).toHaveBeenCalledWith(
        'pubsub.subscribe',
        expect.objectContaining({
          topics: ['test.*'],
          subscriber_id: 'test-sub'
        })
      );
    });

    it('should support single topic subscription', () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const observable = pubsub.subscribeTopic('specific.topic');

      expect(observable).toBeDefined();
    });
  });
});

