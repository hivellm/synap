/**
 * Event Stream Patterns Example
 * 
 * Demonstrates reactive event stream consumption patterns
 */

import { Synap } from '../src/index';
import { filter, map, bufferTime } from 'rxjs/operators';

interface ChatMessage {
  user: string;
  text: string;
  timestamp: number;
}

interface UserAction {
  userId: string;
  action: 'login' | 'logout';
  timestamp: number;
}

/**
 * Example 1: Basic Event Stream
 */
async function basicEventStream() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  console.log('📡 Pattern 1: Basic Event Stream\n');

  const ROOM = 'chat-room';
  
  // Create room
  await synap.stream.createRoom(ROOM);

  // Publish some events
  await synap.stream.publish(ROOM, 'message.sent', {
    user: 'Alice',
    text: 'Hello, World!',
    timestamp: Date.now(),
  });

  await synap.stream.publish(ROOM, 'message.sent', {
    user: 'Bob',
    text: 'Hi Alice!',
    timestamp: Date.now(),
  });

  // Consume reactively
  synap.stream.consume$<ChatMessage>({
    roomName: ROOM,
    subscriberId: 'chat-viewer',
    fromOffset: 0,
    pollingInterval: 500,
  }).subscribe({
    next: (event) => {
      console.log(`[${event.offset}] ${event.data.user}: ${event.data.text}`);
    },
    error: (err) => console.error('Error:', err)
  });

  // Publish more messages
  setTimeout(async () => {
    await synap.stream.publish(ROOM, 'message.sent', {
      user: 'Charlie',
      text: 'Anyone here?',
      timestamp: Date.now(),
    });
  }, 1000);

  // Stop after 3 seconds
  setTimeout(() => {
    synap.stream.stopConsumer(ROOM, 'chat-viewer');
    synap.close();
  }, 3000);
}

/**
 * Example 2: Event Filtering
 */
async function eventFiltering() {
  const synap = new Synap();

  console.log('🔍 Pattern 2: Event Filtering\n');

  const ROOM = 'user-events';
  await synap.stream.createRoom(ROOM);

  // Publish mixed events
  await synap.stream.publish(ROOM, 'user.login', { userId: '1', action: 'login' });
  await synap.stream.publish(ROOM, 'user.logout', { userId: '2', action: 'logout' });
  await synap.stream.publish(ROOM, 'user.login', { userId: '3', action: 'login' });

  // Filter only login events
  synap.stream.consumeEvent$<UserAction>({
    roomName: ROOM,
    subscriberId: 'login-monitor',
    fromOffset: 0,
    eventName: 'user.login',
  }).subscribe({
    next: (event) => {
      console.log(`✅ User ${event.data.userId} logged in`);
    }
  });

  setTimeout(() => {
    synap.stream.stopAllConsumers();
    synap.close();
  }, 2000);
}

/**
 * Example 3: Event Aggregation with Batching
 */
async function eventAggregation() {
  const synap = new Synap();

  console.log('📊 Pattern 3: Event Aggregation\n');

  const ROOM = 'analytics';
  await synap.stream.createRoom(ROOM);

  // Publish analytics events rapidly
  for (let i = 0; i < 10; i++) {
    await synap.stream.publish(ROOM, 'page.view', {
      page: `/page${i}`,
      timestamp: Date.now(),
    });
  }

  // Aggregate events into batches
  synap.stream.consume$<{ page: string }>({
    roomName: ROOM,
    subscriberId: 'analytics-aggregator',
    fromOffset: 0,
    pollingInterval: 100,
  }).pipe(
    bufferTime(1000),
    map(events => ({
      count: events.length,
      pages: events.map(e => e.data.page),
    }))
  ).subscribe({
    next: (batch) => {
      if (batch.count > 0) {
        console.log(`Batch: ${batch.count} page views`);
        console.log(`Pages: ${batch.pages.join(', ')}`);
      }
    }
  });

  setTimeout(() => {
    synap.stream.stopAllConsumers();
    synap.close();
  }, 3000);
}

/**
 * Example 4: Real-time Monitoring
 */
async function realtimeMonitoring() {
  const synap = new Synap();

  console.log('📈 Pattern 4: Real-time Monitoring\n');

  const ROOM = 'system-metrics';
  await synap.stream.createRoom(ROOM);

  // Monitor stream stats
  synap.stream.stats$(ROOM, 1000).subscribe({
    next: (stats) => {
      console.log(`Events: ${stats.event_count}, Subscribers: ${stats.subscribers}`);
    }
  });

  // Publish metrics periodically
  const interval = setInterval(async () => {
    await synap.stream.publish(ROOM, 'metric.cpu', {
      value: Math.random() * 100,
      timestamp: Date.now(),
    });
  }, 500);

  // Stop after 5 seconds
  setTimeout(() => {
    clearInterval(interval);
    synap.stream.stopAllConsumers();
    synap.close();
  }, 5000);
}

/**
 * Example 5: Event Replay from Offset
 */
async function eventReplay() {
  const synap = new Synap();

  console.log('⏮️  Pattern 5: Event Replay\n');

  const ROOM = 'audit-log';
  await synap.stream.createRoom(ROOM);

  // Publish some events
  for (let i = 0; i < 5; i++) {
    await synap.stream.publish(ROOM, 'action.performed', {
      action: `Action ${i}`,
      timestamp: Date.now(),
    });
  }

  // Get current stats
  const stats = await synap.stream.stats(ROOM);
  console.log(`Total events in stream: ${stats.event_count}`);

  // Replay from beginning
  console.log('\n📜 Replaying from offset 0:');
  
  synap.stream.consume$({
    roomName: ROOM,
    subscriberId: 'replay-consumer',
    fromOffset: 0, // Start from beginning
    pollingInterval: 100,
  }).subscribe({
    next: (event) => {
      console.log(`  [${event.offset}] ${event.event}: ${JSON.stringify(event.data)}`);
    }
  });

  setTimeout(() => {
    synap.stream.stopAllConsumers();
    synap.close();
  }, 2000);
}

/**
 * Example 6: Multi-Room Consumption
 */
async function multiRoom() {
  const synap = new Synap();

  console.log('🏠 Pattern 6: Multi-Room Consumption\n');

  const ROOMS = ['room-a', 'room-b', 'room-c'];
  
  // Create rooms
  for (const room of ROOMS) {
    await synap.stream.createRoom(room);
  }

  // Publish to each room
  for (const room of ROOMS) {
    await synap.stream.publish(room, 'event.test', {
      room,
      data: `Event from ${room}`,
    });
  }

  // Consume from all rooms
  ROOMS.forEach((room) => {
    synap.stream.consume$({
      roomName: room,
      subscriberId: `consumer-${room}`,
      fromOffset: 0,
    }).subscribe({
      next: (event) => {
        console.log(`[${room}] ${event.event}: ${event.data.data}`);
      }
    });
  });

  setTimeout(() => {
    synap.stream.stopAllConsumers();
    synap.close();
  }, 2000);
}

/**
 * Example 7: Error-Resilient Stream
 */
async function errorResilient() {
  const synap = new Synap();

  console.log('🛡️  Pattern 7: Error-Resilient Stream\n');

  const ROOM = 'resilient-stream';
  await synap.stream.createRoom(ROOM);

  // Publish events
  await synap.stream.publish(ROOM, 'data.received', { value: 'valid' });

  // Consume with error handling
  synap.stream.consume$<{ value: string }>({
    roomName: ROOM,
    subscriberId: 'resilient-consumer',
    fromOffset: 0,
    pollingInterval: 500,
  }).pipe(
    filter(event => {
      // Validate event data
      return event.data && typeof event.data.value === 'string';
    })
  ).subscribe({
    next: (event) => {
      console.log(`✅ Valid event: ${event.data.value}`);
    },
    error: (err) => {
      console.error('❌ Stream error:', err);
    }
  });

  setTimeout(() => {
    synap.stream.stopAllConsumers();
    synap.close();
  }, 2000);
}

// Run examples
const pattern = process.argv[2] || '1';

switch (pattern) {
  case '1':
    basicEventStream().catch(console.error);
    break;
  case '2':
    eventFiltering().catch(console.error);
    break;
  case '3':
    eventAggregation().catch(console.error);
    break;
  case '4':
    realtimeMonitoring().catch(console.error);
    break;
  case '5':
    eventReplay().catch(console.error);
    break;
  case '6':
    multiRoom().catch(console.error);
    break;
  case '7':
    errorResilient().catch(console.error);
    break;
  default:
    console.log('Usage: ts-node stream-patterns.ts [1-7]');
    console.log('  1 - Basic Event Stream');
    console.log('  2 - Event Filtering');
    console.log('  3 - Event Aggregation');
    console.log('  4 - Real-time Monitoring');
    console.log('  5 - Event Replay');
    console.log('  6 - Multi-Room Consumption');
    console.log('  7 - Error-Resilient Stream');
}

