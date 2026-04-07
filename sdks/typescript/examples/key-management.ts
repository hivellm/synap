/**
 * Key Management Operations Examples
 * 
 * Demonstrates key management operations: EXISTS, TYPE, RENAME, COPY, RANDOMKEY
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runKeyManagementExamples() {
  console.log('🔑 === KEY MANAGEMENT OPERATIONS EXAMPLES ===\n');

  try {
    // EXISTS (via key.exists command)
    const existsResult = await synap.getClient().sendCommand<{ exists: boolean }>('key.exists', { key: 'user:1' });
    console.log('✅ EXISTS user:1:', existsResult.exists);

    // TYPE (via key.type command)
    const typeResult = await synap.getClient().sendCommand<{ type: string }>('key.type', { key: 'user:1' });
    console.log('✅ TYPE user:1:', typeResult.type);

    // RENAME (via key.rename command)
    await synap.kv.set('old-key', 'value');
    await synap.getClient().sendCommand('key.rename', { source: 'old-key', destination: 'new-key' });
    const renamedValue = await synap.kv.get('new-key');
    console.log('✅ RENAME:', renamedValue);

    // COPY (via key.copy command)
    await synap.kv.set('source-key', 'source-value');
    await synap.getClient().sendCommand('key.copy', { source: 'source-key', destination: 'dest-key' });
    const copiedValue = await synap.kv.get('dest-key');
    console.log('✅ COPY:', copiedValue);

    // RANDOMKEY (via key.randomkey command)
    const randomKeyResult = await synap.getClient().sendCommand<{ key: string | null }>('key.randomkey', {});
    console.log('✅ RANDOMKEY:', randomKeyResult.key);

    console.log('\n✅ Key Management operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runKeyManagementExamples().catch(console.error);

