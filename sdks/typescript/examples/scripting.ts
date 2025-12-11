/**
 * Scripting Operations Examples
 * 
 * Demonstrates Lua scripting operations: EVAL, LOAD, EVALSHA, EXISTS, FLUSH
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runScriptingExamples() {
  console.log('📜 === SCRIPTING OPERATIONS EXAMPLES ===\n');

  try {
    // EVAL
    const scriptResult = await synap.script.eval(
      `return tonumber(ARGV[1]) + tonumber(ARGV[2])`,
      { keys: [], args: ['10', '20'] }
    );
    console.log('✅ EVAL script result:', scriptResult.result);

    // LOAD
    const script = `return KEYS[1] .. ":" .. ARGV[1]`;
    const sha = await synap.script.load(script);
    console.log('✅ LOAD script SHA:', sha);

    // EVALSHA
    const evalshaResult = await synap.script.evalsha(sha, { keys: ['key1'], args: ['value1'] });
    console.log('✅ EVALSHA result:', evalshaResult.result);

    // EXISTS
    const scriptExists = await synap.script.exists([sha]);
    console.log('✅ SCRIPT EXISTS:', scriptExists[0]);

    // FLUSH
    await synap.script.flush();
    console.log('✅ FLUSH scripts');

    console.log('\n✅ Scripting operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runScriptingExamples().catch(console.error);

