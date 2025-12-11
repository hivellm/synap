/**
 * Hash Operations Examples
 * 
 * Demonstrates hash data structure operations: HSET, HGET, HGETALL, HMSET, HINCRBY, HLEN, HDEL
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runHashExamples() {
  console.log('🗂️  === HASH OPERATIONS EXAMPLES ===\n');

  try {
    // HSET
    await synap.hash.set('user:profile:1', 'name', 'Bob');
    await synap.hash.set('user:profile:1', 'age', '25');
    await synap.hash.set('user:profile:1', 'city', 'New York');
    console.log('✅ HSET multiple fields');

    // HGET
    const name = await synap.hash.get('user:profile:1', 'name');
    console.log('✅ HGET name:', name);

    // HGETALL
    const profile = await synap.hash.getAll('user:profile:1');
    console.log('✅ HGETALL:', profile);

    // HMSET
    await synap.hash.mset('user:profile:2', {
      name: 'Charlie',
      age: '35',
      city: 'London',
    });
    console.log('✅ HMSET');

    // HINCRBY (works with new field or existing numeric field)
    await synap.hash.incrBy('user:profile:1', 'visits', 5);
    const visits = await synap.hash.get('user:profile:1', 'visits');
    console.log('✅ HINCRBY visits:', visits);

    // HLEN
    const hashLen = await synap.hash.len('user:profile:1');
    console.log('✅ HLEN:', hashLen);

    // HDEL (server expects fields array)
    await synap.getClient().sendCommand('hash.del', { 
      key: 'user:profile:1', 
      fields: ['city'] 
    });
    console.log('✅ HDEL city field');

    // STATS (via hash.stats command)
    const hashStats = await synap.getClient().sendCommand('hash.stats', {});
    console.log('✅ Hash Stats:', hashStats);

    console.log('\n✅ Hash operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runHashExamples().catch(console.error);

