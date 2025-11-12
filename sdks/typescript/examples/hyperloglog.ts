/**
 * HyperLogLog Operations Examples
 * 
 * Demonstrates HyperLogLog operations: PFADD, PFCOUNT, PFMERGE
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runHyperLogLogExamples() {
  console.log('📊 === HYPERLOGLOG OPERATIONS EXAMPLES ===\n');

  try {
    // PFADD
    await synap.hyperloglog.pfadd('unique-visitors', ['user1', 'user2', 'user3', 'user1']);
    console.log('✅ PFADD unique visitors');

    // PFCOUNT
    const count = await synap.hyperloglog.pfcount('unique-visitors');
    console.log('✅ PFCOUNT:', count);

    // PFMERGE
    await synap.hyperloglog.pfadd('visitors-day1', ['user1', 'user2']);
    await synap.hyperloglog.pfadd('visitors-day2', ['user2', 'user3']);
    await synap.hyperloglog.pfmerge('visitors-total', ['visitors-day1', 'visitors-day2']);
    const totalCount = await synap.hyperloglog.pfcount('visitors-total');
    console.log('✅ PFMERGE total:', totalCount);

    // STATS
    const hllStats = await synap.hyperloglog.stats();
    console.log('✅ HyperLogLog Stats:', hllStats);

    console.log('\n✅ HyperLogLog operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runHyperLogLogExamples().catch(console.error);

