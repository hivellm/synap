/**
 * Pub/Sub Patterns Example
 * 
 * Demonstrates reactive pub/sub subscription patterns
 */

import { Synap } from '../src/index';

interface UserEvent {
  userId: string;
  action: string;
  timestamp: number;
}

interface Notification {
  type: string;
  message: string;
  priority: 'low' | 'medium' | 'high';
}

/**
 * Example 1: Basic Pub/Sub
 */
async function basicPubSub() {
  const synap = new Synap({
    url: process.env.SYNAP_URL || 'http://localhost:15500',
  });

  console.log('📢 Pattern 1: Basic Pub/Sub\n');

  // Publish messages to topics
  await synap.pubsub.publish('user.created', {
    userId: '123',
    name: 'Alice',
    email: 'alice@example.com',
  });

  await synap.pubsub.publish('user.updated', {
    userId: '123',
    name: 'Alice Smith',
  });

  console.log('✅ Published messages to topics');

  // Subscribe to topics (Note: This requires WebSocket support)
  // synap.pubsub.subscribe({
  //   topics: ['user.created', 'user.updated'],
  //   subscriberId: 'user-subscriber'
  // }).subscribe({
  //   next: (message) => {
  //     console.log(`Topic: ${message.topic}`);
  //     console.log(`Data:`, message.data);
  //   }
  // });

  setTimeout(() => {
    synap.close();
  }, 1000);
}

/**
 * Example 2: Topic-based Publishing
 */
async function topicPublishing() {
  const synap = new Synap();

  console.log('🏷️  Pattern 2: Topic-based Publishing\n');

  // Publish to different topics
  const topics = [
    { topic: 'notifications.email', data: { to: 'user@example.com', subject: 'Welcome!' } },
    { topic: 'notifications.sms', data: { to: '+1234567890', text: 'Welcome!' } },
    { topic: 'notifications.push', data: { device: 'abc123', title: 'Welcome!' } },
  ];

  for (const { topic, data } of topics) {
    await synap.pubsub.publish(topic, data);
    console.log(`✅ Published to ${topic}`);
  }

  synap.close();
}

/**
 * Example 3: Priority Messages
 */
async function priorityMessages() {
  const synap = new Synap();

  console.log('⚡ Pattern 3: Priority Messages\n');

  // Publish with different priorities
  await synap.pubsub.publish('alerts.system', {
    type: 'info',
    message: 'System update available',
  }, { priority: 1 });

  await synap.pubsub.publish('alerts.system', {
    type: 'warning',
    message: 'High memory usage',
  }, { priority: 5 });

  await synap.pubsub.publish('alerts.system', {
    type: 'error',
    message: 'Service unavailable',
  }, { priority: 9 });

  console.log('✅ Published alerts with priorities');

  synap.close();
}

/**
 * Example 4: Wildcard Topic Subscription
 */
async function wildcardTopics() {
  const synap = new Synap();

  console.log('🌟 Pattern 4: Wildcard Topics\n');

  // Note: Server must support wildcard subscriptions
  // Example patterns:
  // - 'user.*' - matches user.created, user.updated, user.deleted
  // - '*.error' - matches app.error, db.error, api.error
  // - 'app.*.event' - matches app.user.event, app.order.event

  console.log('Wildcard topic patterns:');
  console.log('  user.* - All user events');
  console.log('  *.error - All error events');
  console.log('  app.*.event - All app events');

  // Subscribe with wildcard (requires WebSocket)
  // synap.pubsub.subscribe({
  //   topics: ['user.*', '*.error'],
  //   subscriberId: 'wildcard-subscriber'
  // }).subscribe({
  //   next: (message) => {
  //     console.log(`Matched topic: ${message.topic}`);
  //     console.log(`Data:`, message.data);
  //   }
  // });

  synap.close();
}

/**
 * Example 5: Single Topic Subscription
 */
async function singleTopicSubscription() {
  const synap = new Synap();

  console.log('📝 Pattern 5: Single Topic Subscription\n');

  // Publish to topic
  await synap.pubsub.publish('orders.created', {
    orderId: 'ORDER-123',
    total: 99.99,
    timestamp: Date.now(),
  });

  console.log('✅ Published order event');

  // Subscribe to single topic (requires WebSocket)
  // synap.pubsub.subscribeTopic('orders.created').subscribe({
  //   next: (message) => {
  //     console.log('New order:', message.data);
  //   }
  // });

  synap.close();
}

/**
 * Example 6: Message Broadcasting
 */
async function messageBroadcasting() {
  const synap = new Synap();

  console.log('📡 Pattern 6: Message Broadcasting\n');

  // Broadcast system-wide announcements
  const announcement = {
    title: 'System Maintenance',
    message: 'Scheduled maintenance tonight at 2 AM',
    startTime: new Date('2025-10-24T02:00:00Z').toISOString(),
    duration: '2 hours',
  };

  await synap.pubsub.publish('system.announcements', announcement);
  
  console.log('📢 Broadcast announcement to all subscribers');

  synap.close();
}

/**
 * Example 7: Event-Driven Architecture
 */
async function eventDrivenArchitecture() {
  const synap = new Synap();

  console.log('🏗️  Pattern 7: Event-Driven Architecture\n');

  // Simulate microservices publishing events

  // User Service publishes user events
  await synap.pubsub.publish('user-service.user.created', {
    userId: '123',
    timestamp: Date.now(),
  });

  // Order Service reacts to user creation
  await synap.pubsub.publish('order-service.cart.created', {
    userId: '123',
    cartId: 'CART-456',
    timestamp: Date.now(),
  });

  // Notification Service sends welcome email
  await synap.pubsub.publish('notification-service.email.sent', {
    userId: '123',
    type: 'welcome',
    timestamp: Date.now(),
  });

  console.log('✅ Event-driven flow completed');
  console.log('  1. User created');
  console.log('  2. Cart initialized');
  console.log('  3. Welcome email sent');

  synap.close();
}

// Run examples
const pattern = process.argv[2] || '1';

switch (pattern) {
  case '1':
    basicPubSub().catch(console.error);
    break;
  case '2':
    topicPublishing().catch(console.error);
    break;
  case '3':
    priorityMessages().catch(console.error);
    break;
  case '4':
    wildcardTopics().catch(console.error);
    break;
  case '5':
    singleTopicSubscription().catch(console.error);
    break;
  case '6':
    messageBroadcasting().catch(console.error);
    break;
  case '7':
    eventDrivenArchitecture().catch(console.error);
    break;
  default:
    console.log('Usage: ts-node pubsub-patterns.ts [1-7]');
    console.log('  1 - Basic Pub/Sub');
    console.log('  2 - Topic-based Publishing');
    console.log('  3 - Priority Messages');
    console.log('  4 - Wildcard Topics');
    console.log('  5 - Single Topic Subscription');
    console.log('  6 - Message Broadcasting');
    console.log('  7 - Event-Driven Architecture');
}

