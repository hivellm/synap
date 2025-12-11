/**
 * Run All Examples Script
 * 
 * This script runs all feature examples and tests them against the Docker server
 * 
 * Usage: npx tsx examples/run-all-examples.ts
 */

import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

async function main() {
  console.log('🚀 Running all Synap TypeScript SDK examples...\n');

  try {
    // Check if server is running
    console.log('📡 Checking server connection...');
    const healthCheck = await fetch('http://localhost:15500/health');
    if (!healthCheck.ok) {
      throw new Error('Server is not running. Please start the Docker container first.');
    }
    const health = await healthCheck.json();
    console.log('✅ Server is running:', health);
    console.log('');

    // Run all features example
    console.log('📦 Running all-features.ts...\n');
    const { stdout, stderr } = await execAsync('npx tsx examples/all-features.ts', {
      cwd: process.cwd(),
    });

    if (stdout) {
      console.log(stdout);
    }
    if (stderr) {
      console.error(stderr);
    }

    console.log('\n✅ All examples completed successfully!');
  } catch (error: any) {
    console.error('❌ Error running examples:', error.message);
    if (error.stdout) console.log(error.stdout);
    if (error.stderr) console.error(error.stderr);
    process.exit(1);
  }
}

main();

