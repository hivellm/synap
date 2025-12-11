"""Authentication Examples for Synap Python SDK

This example demonstrates how to use authentication with the Synap Python SDK,
including Basic Auth and API Key authentication.
"""

import asyncio
from synap_sdk import SynapClient, SynapConfig


async def example_basic_auth():
    """Example: Using Basic Auth (username/password)"""
    print("=== Basic Auth Example ===\n")
    
    # Create config with Basic Auth credentials
    config = SynapConfig(
        "http://localhost:15500",
        username="root",
        password="root"
    )
    
    async with SynapClient(config) as client:
        # Test connection
        print("Testing connection with Basic Auth...")
        
        # Perform operations - authentication is automatic
        await client.kv.set("test:key", "test_value")
        value = await client.kv.get("test:key")
        print(f"✅ Successfully set and retrieved value: {value}")
        
        # Clean up
        await client.kv.delete("test:key")
        print("✅ Cleaned up test key\n")


async def example_api_key_auth():
    """Example: Using API Key authentication"""
    print("=== API Key Authentication Example ===\n")
    
    # First, create an API key using Basic Auth
    config_with_auth = SynapConfig(
        "http://localhost:15500",
        username="root",
        password="root"
    )
    
    async with SynapClient(config_with_auth) as admin_client:
        # Create an API key (this would typically be done via REST API)
        # For this example, we'll assume you have an API key
        api_key = "your-api-key-here"  # Replace with actual API key
        
        print(f"Using API key: {api_key[:10]}...")
    
    # Now use the API key for authentication
    config_with_key = SynapConfig(
        "http://localhost:15500",
        auth_token=api_key
    )
    
    async with SynapClient(config_with_key) as client:
        # Perform operations - API key authentication is automatic
        await client.kv.set("test:api_key", "test_value")
        value = await client.kv.get("test:api_key")
        print(f"✅ Successfully set and retrieved value with API key: {value}")
        
        # Clean up
        await client.kv.delete("test:api_key")
        print("✅ Cleaned up test key\n")


async def example_builder_pattern():
    """Example: Using builder pattern for authentication"""
    print("=== Builder Pattern Example ===\n")
    
    # Create base config
    config = SynapConfig.create("http://localhost:15500")
    
    # Add authentication using builder pattern
    config_with_auth = config.with_basic_auth("root", "root")
    
    async with SynapClient(config_with_auth) as client:
        await client.kv.set("test:builder", "test_value")
        value = await client.kv.get("test:builder")
        print(f"✅ Successfully used builder pattern: {value}")
        
        await client.kv.delete("test:builder")
        print("✅ Cleaned up test key\n")


async def example_switch_auth_methods():
    """Example: Switching between authentication methods"""
    print("=== Switching Auth Methods Example ===\n")
    
    base_config = SynapConfig.create("http://localhost:15500")
    
    # Start with Basic Auth
    basic_config = base_config.with_basic_auth("root", "root")
    async with SynapClient(basic_config) as client:
        await client.kv.set("test:switch", "basic_auth")
        print("✅ Set value using Basic Auth")
    
    # Switch to API Key (if you have one)
    # api_key_config = base_config.with_auth_token("your-api-key")
    # async with SynapClient(api_key_config) as client:
    #     value = await client.kv.get("test:switch")
    #     print(f"✅ Retrieved value using API Key: {value}")
    
    # Clean up
    async with SynapClient(basic_config) as client:
        await client.kv.delete("test:switch")
        print("✅ Cleaned up test key\n")


async def main():
    """Run all authentication examples"""
    print("🔐 Synap Python SDK - Authentication Examples\n")
    print("=" * 50 + "\n")
    
    try:
        # Run examples
        await example_basic_auth()
        await example_api_key_auth()
        await example_builder_pattern()
        await example_switch_auth_methods()
        
        print("=" * 50)
        print("✅ All authentication examples completed successfully!")
        
    except Exception as e:
        print(f"❌ Error: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    asyncio.run(main())

