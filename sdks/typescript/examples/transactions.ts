/**
 * Transaction Operations Examples
 * 
 * Demonstrates transaction operations: WATCH, MULTI, EXEC, UNWATCH
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runTransactionExamples() {
  console.log('🔄 === TRANSACTION OPERATIONS EXAMPLES ===\n');

  try {
    const txClientId = `tx-${Date.now()}`;
    
    // WATCH (creates transaction implicitly)
    await synap.transaction.watch({ keys: ['user:1', 'user:2'], clientId: txClientId });
    console.log('✅ WATCH keys (transaction created implicitly)');

    // Queue commands (need to pass clientId in options)
    // Note: SDK doesn't support clientId in kv.set options yet, so we'll use sendCommand directly
    await synap.getClient().sendCommand('kv.set', { 
      key: 'user:1', 
      value: { balance: 100 },
      client_id: txClientId 
    });
    await synap.getClient().sendCommand('kv.set', { 
      key: 'user:2', 
      value: { balance: 50 },
      client_id: txClientId 
    });
    console.log('✅ Queued commands in transaction');

    // EXEC
    const execResult = await synap.transaction.exec({ clientId: txClientId });
    if (!execResult.success) {
      console.log('❌ Transaction aborted (watched keys changed)');
    } else {
      console.log('✅ EXEC transaction:', execResult.results?.length || 0, 'commands executed');
    }

    // UNWATCH
    await synap.transaction.unwatch({ clientId: txClientId });
    console.log('✅ UNWATCH');

    console.log('\n✅ Transaction operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runTransactionExamples().catch(console.error);

