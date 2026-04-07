/**
 * List Operations Examples
 * 
 * Demonstrates list data structure operations: LPUSH, RPUSH, LRANGE, LLEN, LPOP, RPOP, LINDEX
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runListExamples() {
  console.log('📋 === LIST OPERATIONS EXAMPLES ===\n');

  try {
    // LPUSH
    await synap.list.lpush('tasks', 'task1', 'task2', 'task3');
    console.log('✅ LPUSH 3 tasks');

    // RPUSH
    await synap.list.rpush('tasks', 'task4', 'task5');
    console.log('✅ RPUSH 2 tasks');

    // LLEN (using list.llen command)
    const llenResult = await synap.getClient().sendCommand<{ length: number }>('list.llen', { key: 'tasks' });
    console.log('✅ LLEN:', llenResult.length);

    // LRANGE (using list.lrange command)
    const lrangeResult = await synap.getClient().sendCommand<{ values: string[] }>('list.lrange', { 
      key: 'tasks', 
      start: 0, 
      stop: -1 
    });
    console.log('✅ LRANGE:', lrangeResult.values);

    // LPOP
    const firstTask = await synap.list.lpop('tasks');
    console.log('✅ LPOP:', firstTask);

    // RPOP
    const lastTask = await synap.list.rpop('tasks');
    console.log('✅ RPOP:', lastTask);

    // LINDEX (using list.lindex command)
    const lindexResult = await synap.getClient().sendCommand<{ value: string | null }>('list.lindex', { 
      key: 'tasks', 
      index: 0 
    });
    console.log('✅ LINDEX 0:', lindexResult.value);

    // STATS (via list.stats command)
    const listStats = await synap.getClient().sendCommand('list.stats', {});
    console.log('✅ List Stats:', listStats);

    console.log('\n✅ List operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runListExamples().catch(console.error);

