/**
 * Pub/Sub Operations Examples
 * 
 * Demonstrates publish/subscribe operations: PUBLISH, STATS, LIST TOPICS
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runPubSubExamples() {
  console.log('📢 === PUB/SUB OPERATIONS EXAMPLES ===\n');

  try {
    // PUBLISH
    await synap.pubsub.publish('user.created', { id: 1, name: 'Alice' });
    await synap.pubsub.publish('user.updated', { id: 1, name: 'Alice Updated' });
    await synap.pubsub.publish('order.placed', { orderId: 123, amount: 99.99 });
    console.log('✅ PUBLISH to topics');

    // STATS (pubsub.stats requires a topic)
    const pubsubStats = await synap.pubsub.stats('user.created');
    console.log('✅ PubSub Stats:', pubsubStats);

    // LIST TOPICS (using pubsub.topics command)
    const topicsResult = await synap.getClient().sendCommand<{ topics: string[] }>('pubsub.topics', {});
    console.log('✅ LIST TOPICS:', topicsResult.topics);

    console.log('\n✅ Pub/Sub operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runPubSubExamples().catch(console.error);

