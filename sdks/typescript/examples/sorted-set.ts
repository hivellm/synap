/**
 * Sorted Set Operations Examples
 * 
 * Demonstrates sorted set data structure operations: ZADD, ZCARD, ZRANGE, ZRANK, ZSCORE, ZINCRBY
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runSortedSetExamples() {
  console.log('⭐ === SORTED SET OPERATIONS EXAMPLES ===\n');

  try {
    // ZADD
    await synap.sortedSet.add('leaderboard', 'player1', 100);
    await synap.sortedSet.add('leaderboard', 'player2', 200);
    await synap.sortedSet.add('leaderboard', 'player3', 150);
    console.log('✅ ZADD 3 players');

    // ZCARD
    const leaderboardSize = await synap.sortedSet.card('leaderboard');
    console.log('✅ ZCARD:', leaderboardSize);

    // ZRANGE
    const topPlayers = await synap.sortedSet.range('leaderboard', 0, -1, true);
    console.log('✅ ZRANGE:', topPlayers);

    // ZRANK
    const rank = await synap.sortedSet.rank('leaderboard', 'player2');
    console.log('✅ ZRANK player2:', rank);

    // ZSCORE
    const score = await synap.sortedSet.score('leaderboard', 'player2');
    console.log('✅ ZSCORE player2:', score);

    // ZINCRBY
    await synap.sortedSet.incrBy('leaderboard', 'player1', 50);
    const newScore = await synap.sortedSet.score('leaderboard', 'player1');
    console.log('✅ ZINCRBY player1 +50:', newScore);

    // STATS
    const sortedSetStats = await synap.sortedSet.stats();
    console.log('✅ Sorted Set Stats:', sortedSetStats);

    console.log('\n✅ Sorted Set operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runSortedSetExamples().catch(console.error);

