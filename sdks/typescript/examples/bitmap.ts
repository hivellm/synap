/**
 * Bitmap Operations Examples
 * 
 * Demonstrates bitmap operations: SETBIT, GETBIT, BITCOUNT, BITPOS
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runBitmapExamples() {
  console.log('🔲 === BITMAP OPERATIONS EXAMPLES ===\n');

  try {
    // SETBIT
    await synap.bitmap.setbit('user:online', 0, 1);
    await synap.bitmap.setbit('user:online', 5, 1);
    await synap.bitmap.setbit('user:online', 10, 1);
    console.log('✅ SETBIT user:online');

    // GETBIT
    const bit0 = await synap.bitmap.getbit('user:online', 0);
    const bit1 = await synap.bitmap.getbit('user:online', 1);
    console.log('✅ GETBIT 0:', bit0, 'GETBIT 1:', bit1);

    // BITCOUNT
    const bitCount = await synap.bitmap.bitcount('user:online');
    console.log('✅ BITCOUNT:', bitCount);

    // BITPOS
    const firstSet = await synap.bitmap.bitpos('user:online', 1);
    console.log('✅ BITPOS first 1:', firstSet);

    // STATS
    const bitmapStats = await synap.bitmap.stats();
    console.log('✅ Bitmap Stats:', bitmapStats);

    console.log('\n✅ Bitmap operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runBitmapExamples().catch(console.error);

