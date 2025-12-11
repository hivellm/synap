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

    it('should generate default subscriber ID if not provided', () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const observable = pubsub.subscribe({ topics: ['test.*'] });

      expect(observable).toBeDefined();
    });

    it('should handle subscription errors', async () => {
      const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      const mockClient = {
        sendCommand: vi.fn().mockRejectedValue(new Error('Subscription failed'))
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const observable = pubsub.subscribe({ topics: ['test.*'], subscriberId: 'test-sub' });

      let errorCaught = false;
      observable.subscribe({
        error: (err) => {
          errorCaught = true;
          expect(err.message).toBe('Subscription failed');
        }
      });

      await new Promise(resolve => setTimeout(resolve, 20));
      
      expect(consoleErrorSpy).toHaveBeenCalled();
      consoleErrorSpy.mockRestore();
    });
  });

  describe('Unsubscribe Methods', () => {
    it('should unsubscribe from topics', async () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      
      // Subscribe first
      pubsub.subscribe({ topics: ['test.*'], subscriberId: 'test-sub' });
      await new Promise(resolve => setTimeout(resolve, 10));

      // Then unsubscribe
      pubsub.unsubscribe('test-sub', ['test.*']);

      await new Promise(resolve => setTimeout(resolve, 10));

      expect(mockClient.sendCommand).toHaveBeenCalledWith(
        'pubsub.unsubscribe',
        expect.objectContaining({
          subscriber_id: 'test-sub',
          topics: ['test.*']
        })
      );
    });

    it('should handle unsubscribe when not subscribed', () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      
      expect(() => pubsub.unsubscribe('unknown-sub', ['test.*'])).not.toThrow();
    });

    it('should unsubscribe all subscriptions', async () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ success: true })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      
      // Create multiple subscriptions
      pubsub.subscribe({ topics: ['topic1'], subscriberId: 'sub1' });
      pubsub.subscribe({ topics: ['topic2'], subscriberId: 'sub2' });
      pubsub.subscribe({ topics: ['topic3'], subscriberId: 'sub3' });

      await new Promise(resolve => setTimeout(resolve, 10));

      // Unsubscribe all
      pubsub.unsubscribeAll();

      expect(mockClient.sendCommand).toHaveBeenCalled();
    });

    it('should handle unsubscribe errors gracefully', async () => {
      const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      const mockClient = {
        sendCommand: vi.fn().mockRejectedValue(new Error('Unsubscribe failed'))
      } as any;

      const pubsub = new PubSubManager(mockClient);
      
      // Subscribe first
      pubsub.subscribe({ topics: ['test.*'], subscriberId: 'test-sub' });
      
      // Unsubscribe - should not throw
      expect(() => pubsub.unsubscribe('test-sub', ['test.*'])).not.toThrow();

      await new Promise(resolve => setTimeout(resolve, 20));
      
      expect(consoleErrorSpy).toHaveBeenCalled();
      consoleErrorSpy.mockRestore();
    });
  });

  describe('Stats and Topics', () => {
    it('should get topic stats', async () => {
      const mockStats = { topic: 'test.*', subscribers: 5, messages: 100 };
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue(mockStats)
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const stats = await pubsub.stats('test.*');

      expect(mockClient.sendCommand).toHaveBeenCalledWith(
        'pubsub.stats',
        { topic: 'test.*' }
      );
      expect(stats).toEqual(mockStats);
    });

    it('should list all topics', async () => {
      const mockClient = {
        sendCommand: vi.fn().mockResolvedValue({ topics: ['topic1', 'topic2', 'topic3'] })
      } as any;

      const pubsub = new PubSubManager(mockClient);
      const topics = await pubsub.listTopics();

      expect(mockClient.sendCommand).toHaveBeenCalledWith('pubsub.list', {});
      expect(topics).toEqual(['topic1', 'topic2', 'topic3']);
    });
  });
});

