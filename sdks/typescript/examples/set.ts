/**
 * Set Operations Examples
 * 
 * Demonstrates set data structure operations: SADD, SMEMBERS, SCARD, SISMEMBER, SPOP, SREM
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runSetExamples() {
  console.log('🔢 === SET OPERATIONS EXAMPLES ===\n');

  try {
    // SADD
    await synap.set.add('tags', 'javascript', 'typescript', 'nodejs', 'redis');
    console.log('✅ SADD tags');

    // SMEMBERS
    const tags = await synap.set.members('tags');
    console.log('✅ SMEMBERS:', tags);

    // SCARD (using set.size command)
    const sizeResult = await synap.getClient().sendCommand<{ size: number }>('set.size', { key: 'tags' });
    console.log('✅ SCARD:', sizeResult.size);

    // SISMEMBER
    const isMember = await synap.set.isMember('tags', 'typescript');
    console.log('✅ SISMEMBER typescript:', isMember);

    // SPOP
    const popped = await synap.set.pop('tags', 1);
    console.log('✅ SPOP:', popped);

    // SREM
    await synap.set.rem('tags', 'javascript');
    console.log('✅ SREM javascript');

    // STATS (via set.stats command)
    const setStats = await synap.getClient().sendCommand('set.stats', {});
    console.log('✅ Set Stats:', setStats);

    console.log('\n✅ Set operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runSetExamples().catch(console.error);

