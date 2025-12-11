<?php

declare(strict_types=1);

/**
 * Authentication Examples for Synap PHP SDK
 * 
 * This example demonstrates how to use authentication with the Synap PHP SDK,
 * including Basic Auth and API Key authentication.
 */

require_once __DIR__ . '/../vendor/autoload.php';

use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

function exampleBasicAuth(): void
{
    echo "=== Basic Auth Example ===\n\n";
    
    // Create config with Basic Auth credentials
    $config = SynapConfig::create('http://localhost:15500')
        ->withBasicAuth('root', 'root');
    
    $client = new SynapClient($config);
    
    echo "Testing connection with Basic Auth...\n";
    
    // Perform operations - authentication is automatic
    $client->kv()->set('test:key', 'test_value');
    $value = $client->kv()->get('test:key');
    echo "✅ Successfully set and retrieved value: {$value}\n";
    
    // Clean up
    $client->kv()->delete('test:key');
    echo "✅ Cleaned up test key\n\n";
}

function exampleApiKeyAuth(): void
{
    echo "=== API Key Authentication Example ===\n\n";
    
    // Create config with API Key
    $config = SynapConfig::create('http://localhost:15500')
        ->withAuthToken('your-api-key-here'); // Replace with actual API key
    
    $client = new SynapClient($config);
    
    echo "Using API key authentication...\n";
    
    // Perform operations - API key authentication is automatic
    $client->kv()->set('test:api_key', 'test_value');
    $value = $client->kv()->get('test:api_key');
    echo "✅ Successfully set and retrieved value with API key: {$value}\n";
    
    // Clean up
    $client->kv()->delete('test:api_key');
    echo "✅ Cleaned up test key\n\n";
}

function exampleBuilderPattern(): void
{
    echo "=== Builder Pattern Example ===\n\n";
    
    // Create base config
    $config = SynapConfig::create('http://localhost:15500')
        ->withTimeout(30)
        ->withBasicAuth('root', 'root');
    
    $client = new SynapClient($config);
    
    $client->kv()->set('test:builder', 'test_value');
    $value = $client->kv()->get('test:builder');
    echo "✅ Successfully used builder pattern: {$value}\n";
    
    // Clean up
    $client->kv()->delete('test:builder');
    echo "✅ Cleaned up test key\n\n";
}

function exampleSwitchAuthMethods(): void
{
    echo "=== Switching Auth Methods Example ===\n\n";
    
    // Start with Basic Auth
    $basicConfig = SynapConfig::create('http://localhost:15500')
        ->withBasicAuth('root', 'root');
    
    $basicClient = new SynapClient($basicConfig);
    $basicClient->kv()->set('test:switch', 'basic_auth');
    echo "✅ Set value using Basic Auth\n";
    
    // Switch to API Key (if you have one)
    // $apiKeyConfig = SynapConfig::create('http://localhost:15500')
    //     ->withAuthToken('your-api-key');
    // $apiKeyClient = new SynapClient($apiKeyConfig);
    // $value = $apiKeyClient->kv()->get('test:switch');
    // echo "✅ Retrieved value using API Key: {$value}\n";
    
    // Clean up
    $basicClient->kv()->delete('test:switch');
    echo "✅ Cleaned up test key\n\n";
}

// Main execution
echo "🔐 Synap PHP SDK - Authentication Examples\n";
echo str_repeat('=', 50) . "\n\n";

try {
    exampleBasicAuth();
    exampleApiKeyAuth();
    exampleBuilderPattern();
    exampleSwitchAuthMethods();
    
    echo str_repeat('=', 50) . "\n";
    echo "✅ All authentication examples completed successfully!\n";
} catch (Exception $e) {
    echo "❌ Error: {$e->getMessage()}\n";
    exit(1);
}

