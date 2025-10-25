/**
 * Mock SynapClient for unit testing
 */

import { vi } from 'vitest';
import type { SynapClient } from '../../client';

/**
 * Create a mock SynapClient with configurable responses
 */
export function createMockClient(responses: Map<string, any> = new Map()): SynapClient {
  const mockClient = {
    sendCommand: vi.fn(async (command: string, payload: any) => {
      // Return mocked response based on command
      const key = `${command}:${JSON.stringify(payload)}`;
      
      if (responses.has(key)) {
        return responses.get(key);
      }
      
      if (responses.has(command)) {
        const handler = responses.get(command);
        if (typeof handler === 'function') {
          return handler(payload);
        }
        return handler;
      }

      // Default responses by command type
      return getDefaultResponse(command, payload);
    }),

    ping: vi.fn(async () => true),
    health: vi.fn(async () => ({ 
      status: 'healthy', 
      service: 'synap-mock', 
      version: '0.2.0' 
    })),
    close: vi.fn(),
  } as unknown as SynapClient;

  return mockClient;
}

/**
 * Default mock responses for common commands
 */
function getDefaultResponse(command: string, payload: any): any {
  const cmd = command.split('.')[0];
  const action = command.split('.')[1];

  switch (cmd) {
    case 'kv':
      return handleKVCommand(action, payload);
    case 'queue':
      return handleQueueCommand(action, payload);
    case 'stream':
      return handleStreamCommand(action, payload);
    case 'pubsub':
      return handlePubSubCommand(action, payload);
    default:
      return { success: true };
  }
}

/**
 * Mock KV command responses
 */
function handleKVCommand(action: string, payload: any): any {
  const mockStore = new Map<string, any>();

  switch (action) {
    case 'set':
      mockStore.set(payload.key, payload.value);
      return { success: true };
    
    case 'get':
      const value = mockStore.get(payload.key);
      return value !== undefined ? JSON.stringify(value) : null;
    
    case 'del':
      const deleted = mockStore.delete(payload.key);
      return { deleted };
    
    case 'exists':
      return { exists: mockStore.has(payload.key) };
    
    case 'incr':
      const current = mockStore.get(payload.key) || 0;
      const newValue = current + (payload.amount || 1);
      mockStore.set(payload.key, newValue);
      return { value: newValue };
    
    case 'stats':
      return {
        total_keys: mockStore.size,
        total_memory_bytes: 1024,
        operations: { gets: 10, sets: 5, dels: 2, hits: 8, misses: 2 },
        hit_rate: 0.8
      };
    
    default:
      return { success: true };
  }
}

/**
 * Mock Queue command responses
 */
function handleQueueCommand(action: string, payload: any): any {
  const mockQueues = new Map<string, any[]>();
  const messageId = `msg-${Date.now()}-${Math.random()}`;

  switch (action) {
    case 'create':
      return { success: true };
    
    case 'publish':
      return { message_id: messageId };
    
    case 'consume':
      // Return mock message
      return {
        message: {
          id: messageId,
          payload: Array.from(new TextEncoder().encode(JSON.stringify({ mock: 'data' }))),
          priority: 5,
          retry_count: 0,
          max_retries: 3,
        }
      };
    
    case 'ack':
      return { success: true };
    
    case 'nack':
      return { success: true };
    
    case 'stats':
      return {
        depth: 0,
        consumers: 1,
        published: 10,
        consumed: 8,
        acked: 7,
        nacked: 1,
        dead_lettered: 0,
      };
    
    case 'list':
      return { queues: ['test-queue'] };
    
    case 'purge':
      return { purged: 0 };
    
    case 'delete':
      return { deleted: true };
    
    default:
      return { success: true };
  }
}

/**
 * Mock Stream command responses
 */
function handleStreamCommand(action: string, payload: any): any {
  switch (action) {
    case 'create':
      return { success: true };
    
    case 'publish':
      return { offset: Math.floor(Math.random() * 100) };
    
    case 'consume':
      // Return mock events
      const events = [
        {
          offset: payload.from_offset || 0,
          event: 'test.event',
          data: Array.from(new TextEncoder().encode(JSON.stringify({ test: 'data' }))),
          timestamp: Date.now(),
        }
      ];
      return { events };
    
    case 'stats':
      return {
        max_offset: 10,
        subscribers: 1,
        total_events: 10,
        total_consumed: 5,
        room: payload.room || 'test-room',
        created_at: Date.now(),
        last_activity: Date.now(),
      };
    
    case 'list':
      return { rooms: ['test-room'] };
    
    case 'delete':
      return { deleted: payload.room };
    
    default:
      return { success: true };
  }
}

/**
 * Mock PubSub command responses
 */
function handlePubSubCommand(action: string, payload: any): any {
  switch (action) {
    case 'publish':
      return { success: true };
    
    case 'subscribe':
      return { subscriber_id: `sub-${Date.now()}`, subscribed: payload.topics };
    
    case 'unsubscribe':
      return { success: true };
    
    case 'list':
      return { topics: ['test.topic'] };
    
    default:
      return { success: true };
  }
}

/**
 * Create a mock client with predefined responses for a specific scenario
 */
export function createScenarioMock(scenario: 'empty-queue' | 'full-queue' | 'error'): SynapClient {
  const responses = new Map<string, any>();

  switch (scenario) {
    case 'empty-queue':
      responses.set('queue.consume', { message: null });
      break;
    
    case 'full-queue':
      responses.set('queue.consume', (payload: any) => ({
        message: {
          id: `msg-${Date.now()}`,
          payload: Array.from(new TextEncoder().encode(JSON.stringify({ task: 'test' }))),
          priority: 5,
          retry_count: 0,
          max_retries: 3,
        }
      }));
      break;
    
    case 'error':
      responses.set('kv.get', () => {
        throw new Error('Mock error');
      });
      break;
  }

  return createMockClient(responses);
}

