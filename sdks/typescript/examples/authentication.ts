/**
 * Authentication Examples for Synap TypeScript SDK
 * 
 * This example demonstrates how to use authentication with the Synap TypeScript SDK,
 * including Basic Auth and API Key authentication.
 */

import { Synap } from '../src/index';

async function exampleBasicAuth() {
  console.log('=== Basic Auth Example ===\n');
  
  // Create client with Basic Auth
  const synap = new Synap({
    url: 'http://localhost:15500',
    auth: {
      type: 'basic',
      username: 'root',
      password: 'root',
    },
  });
  
  try {
    // Test connection
    console.log('Testing connection with Basic Auth...');
    const health = await synap.health();
    console.log('✅ Server health:', health);
    
    // Perform operations - authentication is automatic
    await synap.kv.set('test:key', 'test_value');
    const value = await synap.kv.get('test:key');
    console.log(`✅ Successfully set and retrieved value: ${value}`);
    
    // Clean up
    await synap.kv.delete('test:key');
    console.log('✅ Cleaned up test key\n');
  } catch (error) {
    console.error('❌ Error:', error);
  }
}

async function exampleApiKeyAuth() {
  console.log('=== API Key Authentication Example ===\n');
  
  // Create client with API Key
  const synap = new Synap({
    url: 'http://localhost:15500',
    auth: {
      type: 'api_key',
      apiKey: 'your-api-key-here', // Replace with actual API key
    },
  });
  
  try {
    console.log('Using API key authentication...');
    
    // Perform operations - API key authentication is automatic
    await synap.kv.set('test:api_key', 'test_value');
    const value = await synap.kv.get('test:api_key');
    console.log(`✅ Successfully set and retrieved value with API key: ${value}`);
    
    // Clean up
    await synap.kv.delete('test:api_key');
    console.log('✅ Cleaned up test key\n');
  } catch (error) {
    console.error('❌ Error:', error);
  }
}

async function exampleNoAuth() {
  console.log('=== No Authentication Example ===\n');
  
  // Create client without authentication (works if auth is disabled on server)
  const synap = new Synap({
    url: 'http://localhost:15500',
  });
  
  try {
    console.log('Testing connection without authentication...');
    const health = await synap.health();
    console.log('✅ Server health:', health);
    
    // Perform operations
    await synap.kv.set('test:no_auth', 'test_value');
    const value = await synap.kv.get('test:no_auth');
    console.log(`✅ Successfully set and retrieved value: ${value}`);
    
    // Clean up
    await synap.kv.delete('test:no_auth');
    console.log('✅ Cleaned up test key\n');
  } catch (error) {
    console.error('❌ Error:', error);
  }
}

async function exampleSwitchAuthMethods() {
  console.log('=== Switching Auth Methods Example ===\n');
  
  // Start with Basic Auth
  const basicAuthClient = new Synap({
    url: 'http://localhost:15500',
    auth: {
      type: 'basic',
      username: 'root',
      password: 'root',
    },
  });
  
  try {
    await basicAuthClient.kv.set('test:switch', 'basic_auth');
    console.log('✅ Set value using Basic Auth');
    
    // Switch to API Key (if you have one)
    // const apiKeyClient = new Synap({
    //   url: 'http://localhost:15500',
    //   auth: {
    //     type: 'api_key',
    //     apiKey: 'your-api-key',
    //   },
    // });
    // const value = await apiKeyClient.kv.get('test:switch');
    // console.log(`✅ Retrieved value using API Key: ${value}`);
    
    // Clean up
    await basicAuthClient.kv.delete('test:switch');
    console.log('✅ Cleaned up test key\n');
  } catch (error) {
    console.error('❌ Error:', error);
  }
}

async function main() {
  console.log('🔐 Synap TypeScript SDK - Authentication Examples\n');
  console.log('='.repeat(50) + '\n');
  
  try {
    // Run examples
    await exampleBasicAuth();
    await exampleApiKeyAuth();
    await exampleNoAuth();
    await exampleSwitchAuthMethods();
    
    console.log('='.repeat(50));
    console.log('✅ All authentication examples completed successfully!');
  } catch (error) {
    console.error('❌ Fatal error:', error);
    process.exit(1);
  }
}

// Run if executed directly
if (require.main === module) {
  main().catch(console.error);
}

export { exampleBasicAuth, exampleApiKeyAuth, exampleNoAuth, exampleSwitchAuthMethods };

