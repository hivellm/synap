/**
 * Run All Examples
 * 
 * Executes all feature examples in sequence
 * 
 * Usage: npx tsx examples/run-all.ts
 */

import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

const examples = [
  'kv-store',
  'hash',
  'list',
  'set',
  'sorted-set',
  'queue',
  'stream',
  'pubsub',
  'transactions',
  'key-management',
  'hyperloglog',
  'bitmap',
  'geospatial',
  'scripting',
];

async function runAllExamples() {
  console.log('🚀 Running all Synap TypeScript SDK examples...\n');

  // Check if server is running
  try {
    const healthCheck = await fetch('http://localhost:15500/health');
    if (!healthCheck.ok) {
      throw new Error('Server is not running. Please start the Docker container first.');
    }
    const health = await healthCheck.json();
    console.log('✅ Server is running:', health);
    console.log('');
  } catch (error: any) {
    console.error('❌ Error checking server:', error.message);
    process.exit(1);
  }

  let successCount = 0;
  let failCount = 0;

  for (const example of examples) {
    console.log(`\n${'='.repeat(60)}`);
    console.log(`Running: ${example}.ts`);
    console.log('='.repeat(60));

    try {
      const { stdout, stderr } = await execAsync(`npx tsx examples/${example}.ts`, {
        cwd: process.cwd(),
      });

      if (stdout) {
        console.log(stdout);
      }
      if (stderr && !stderr.includes('node.exe')) {
        console.error(stderr);
      }

      successCount++;
      console.log(`✅ ${example}.ts completed successfully`);
    } catch (error: any) {
      failCount++;
      console.error(`❌ ${example}.ts failed:`, error.message);
      if (error.stdout) console.log(error.stdout);
      if (error.stderr) console.error(error.stderr);
    }
  }

  console.log(`\n${'='.repeat(60)}`);
  console.log('📊 SUMMARY');
  console.log('='.repeat(60));
  console.log(`Total: ${examples.length}`);
  console.log(`✅ Passed: ${successCount}`);
  console.log(`❌ Failed: ${failCount}`);
  console.log(`Success Rate: ${((successCount / examples.length) * 100).toFixed(1)}%`);
}

runAllExamples().catch(console.error);

