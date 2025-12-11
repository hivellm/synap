/**
 * Key-Value Store Examples
 * 
 * Demonstrates basic key-value operations: SET, GET, DELETE, APPEND, STRLEN, GETRANGE, STATS
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runKVStoreExamples() {
  console.log('📦 === KEY-VALUE STORE EXAMPLES ===\n');

  try {
    // SET/GET
    await synap.kv.set('user:1', { name: 'Alice', age: 30, email: 'alice@example.com' });
    const user = await synap.kv.get('user:1');
    console.log('✅ GET user:1:', user);

    // SET with TTL
    await synap.kv.set('session:abc123', { userId: 1, expiresAt: Date.now() }, { ttl: 3600 });
    console.log('✅ SET with TTL');

    // APPEND (via kv.append command)
    await synap.kv.set('log:app', 'Initial log');
    await synap.getClient().sendCommand('kv.append', { key: 'log:app', value: '\nNew log entry' });
    const log = await synap.kv.get('log:app');
    console.log('✅ APPEND result:', log);

    // STRLEN (via kv.strlen command)
    const strlenResult = await synap.getClient().sendCommand<{ length: number }>('kv.strlen', { key: 'log:app' });
    console.log('✅ STRLEN:', strlenResult.length);

    // GETRANGE (via kv.getrange command)
    const rangeResult = await synap.getClient().sendCommand<{ range: string }>('kv.getrange', { 
      key: 'log:app', 
      start: 0, 
      end: 10 
    });
    console.log('✅ GETRANGE:', rangeResult.range);

    // DELETE
    await synap.kv.del('session:abc123');
    console.log('✅ DELETE session:abc123');

    // STATS
    const kvStats = await synap.kv.stats();
    console.log('✅ KV Stats:', kvStats);

    console.log('\n✅ Key-Value Store examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runKVStoreExamples().catch(console.error);

