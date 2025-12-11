/**
 * Pub/Sub Server-to-Server Integration Tests
 * 
 * These tests verify pub/sub functionality against a real Synap server.
 * Run with: RUN_S2S=true npm test
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { Synap } from '../index';

const SYNAP_URL = process.env.SYNAP_URL || 'http://localhost:15500';
const RUN_S2S = process.env.RUN_S2S === 'true';

describe.skipIf(!RUN_S2S)('PubSubManager - Server-to-Server Tests', () => {
  let synap: Synap;

  beforeAll(() => {
    synap = new Synap({ url: SYNAP_URL });
  });

  afterAll(() => {
    synap.close();
  });

  describe('Basic Pub/Sub Operations', () => {
    it('should publish message to topic', async () => {
      const topic = `test.publish.${Date.now()}`;
      const message = { event: 'test', data: 'test-data' };

      const result = await synap.pubsub.publish(topic, message);
      
      // Should succeed even with 0 subscribers
      expect(typeof result).toBe('boolean');
    });

    it('should publish to multiple topics', async () => {
      const topics = [
        `test.user.created.${Date.now()}`,
        `test.user.updated.${Date.now()}`,
        `test.user.deleted.${Date.now()}`,
      ];

      for (const topic of topics) {
        const result = await synap.pubsub.publish(topic, { topic });
        expect(typeof result).toBe('boolean');
      }
    });

    it('should handle different payload types', async () => {
      const topic = `test.types.${Date.now()}`;

      // String payload
      await expect(synap.pubsub.publish(topic, 'string message')).resolves.toBeDefined();

      // Number payload
      await expect(synap.pubsub.publish(topic, 12345)).resolves.toBeDefined();

      // Object payload
      await expect(synap.pubsub.publish(topic, { key: 'value' })).resolves.toBeDefined();

      // Array payload
      await expect(synap.pubsub.publish(topic, [1, 2, 3])).resolves.toBeDefined();

      // Null payload
      await expect(synap.pubsub.publish(topic, null)).resolves.toBeDefined();
    });

    it('should handle topic with wildcards in name', async () => {
      // Publish to specific topic
      const topic = `test.events.user.123.${Date.now()}`;
      const message = { userId: 123, action: 'login' };

      const result = await synap.pubsub.publish(topic, message);
      expect(typeof result).toBe('boolean');
    });

    it('should handle nested object payloads', async () => {
      const topic = `test.nested.${Date.now()}`;
      const message = {
        user: {
          id: 123,
          profile: {
            name: 'Alice',
            settings: {
              theme: 'dark',
              notifications: true
            }
          }
        },
        timestamp: new Date().toISOString()
      };

      const result = await synap.pubsub.publish(topic, message);
      expect(typeof result).toBe('boolean');
    });

    it('should handle large payloads', async () => {
      const topic = `test.large.${Date.now()}`;
      const largeData = 'x'.repeat(10000); // 10KB
      const message = { data: largeData };

      const result = await synap.pubsub.publish(topic, message);
      expect(typeof result).toBe('boolean');
    });

    it('should handle rapid publishing', async () => {
      const topic = `test.rapid.${Date.now()}`;
      const messageCount = 50;

      const promises = [];
      for (let i = 0; i < messageCount; i++) {
        promises.push(synap.pubsub.publish(topic, { id: i }));
      }

      const results = await Promise.all(promises);
      expect(results).toHaveLength(messageCount);
      results.forEach(result => {
        expect(typeof result).toBe('boolean');
      });
    });
  });

  describe('Edge Cases', () => {
    it('should handle empty topic name gracefully', async () => {
      await expect(synap.pubsub.publish('', { test: 'data' }))
        .rejects.toThrow();
    });

    it('should handle special characters in topic', async () => {
      const topic = `test.special-chars_123.${Date.now()}`;
      const message = { test: 'data' };

      const result = await synap.pubsub.publish(topic, message);
      expect(typeof result).toBe('boolean');
    });

    it('should handle concurrent publishes to same topic', async () => {
      const topic = `test.concurrent.${Date.now()}`;
      
      const promises = Array.from({ length: 10 }, (_, i) => 
        synap.pubsub.publish(topic, { message: i })
      );

      const results = await Promise.all(promises);
      expect(results).toHaveLength(10);
    });
  });
});

