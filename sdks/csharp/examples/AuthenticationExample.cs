using Synap.SDK;
using System;
using System.Threading.Tasks;

namespace Synap.SDK.Examples
{
    /// <summary>
    /// Authentication Examples for Synap C# SDK
    ///
    /// This example demonstrates how to use authentication with the Synap C# SDK,
    /// including Basic Auth and API Key authentication.
    /// </summary>
    class AuthenticationExample
    {
        static async Task Main(string[] args)
        {
            Console.WriteLine("🔐 Synap C# SDK - Authentication Examples\n");
            Console.WriteLine(new string('=', 50));
            Console.WriteLine();

            try
            {
                // Run examples
                await ExampleBasicAuth();
                await ExampleApiKeyAuth();
                await ExampleBuilderPattern();
                await ExampleSwitchAuthMethods();

                Console.WriteLine(new string('=', 50));
                Console.WriteLine("✅ All authentication examples completed successfully!");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"❌ Error: {ex.Message}");
                Console.WriteLine(ex.StackTrace);
                Environment.Exit(1);
            }
        }

        static async Task ExampleBasicAuth()
        {
            Console.WriteLine("=== Basic Auth Example ===\n");

            // Create config with Basic Auth credentials
            var config = SynapConfig.Create("http://localhost:15500")
                .WithBasicAuth("root", "root");

            using var client = new SynapClient(config);

            Console.WriteLine("Testing connection with Basic Auth...");

            // Perform operations - authentication is automatic
            await client.KV.SetAsync("test:key", System.Text.Encoding.UTF8.GetBytes("test_value"));
            var value = await client.KV.GetAsync("test:key");

            if (value != null)
            {
                var valueStr = System.Text.Encoding.UTF8.GetString(value);
                Console.WriteLine($"✅ Successfully set and retrieved value: {valueStr}");
            }

            // Clean up
            await client.KV.DeleteAsync("test:key");
            Console.WriteLine("✅ Cleaned up test key\n");
        }

        static async Task ExampleApiKeyAuth()
        {
            Console.WriteLine("=== API Key Authentication Example ===\n");

            // Create config with API Key
            var config = SynapConfig.Create("http://localhost:15500")
                .WithAuthToken("your-api-key-here"); // Replace with actual API key

            using var client = new SynapClient(config);

            Console.WriteLine("Using API key authentication...");

            // Perform operations - API key authentication is automatic
            await client.KV.SetAsync("test:api_key", System.Text.Encoding.UTF8.GetBytes("test_value"));
            var value = await client.KV.GetAsync("test:api_key");

            if (value != null)
            {
                var valueStr = System.Text.Encoding.UTF8.GetString(value);
                Console.WriteLine($"✅ Successfully set and retrieved value with API key: {valueStr}");
            }

            // Clean up
            await client.KV.DeleteAsync("test:api_key");
            Console.WriteLine("✅ Cleaned up test key\n");
        }

        static async Task ExampleBuilderPattern()
        {
            Console.WriteLine("=== Builder Pattern Example ===\n");

            // Create base config
            var config = SynapConfig.Create("http://localhost:15500")
                .WithTimeout(30)
                .WithBasicAuth("root", "root");

            using var client = new SynapClient(config);

            await client.KV.SetAsync("test:builder", System.Text.Encoding.UTF8.GetBytes("test_value"));
            var value = await client.KV.GetAsync("test:builder");

            if (value != null)
            {
                var valueStr = System.Text.Encoding.UTF8.GetString(value);
                Console.WriteLine($"✅ Successfully used builder pattern: {valueStr}");
            }

            // Clean up
            await client.KV.DeleteAsync("test:builder");
            Console.WriteLine("✅ Cleaned up test key\n");
        }

        static async Task ExampleSwitchAuthMethods()
        {
            Console.WriteLine("=== Switching Auth Methods Example ===\n");

            // Start with Basic Auth
            var basicConfig = SynapConfig.Create("http://localhost:15500")
                .WithBasicAuth("root", "root");

            using var basicClient = new SynapClient(basicConfig);
            await basicClient.KV.SetAsync("test:switch", System.Text.Encoding.UTF8.GetBytes("basic_auth"));
            Console.WriteLine("✅ Set value using Basic Auth");

            // Switch to API Key (if you have one)
            // var apiKeyConfig = SynapConfig.Create("http://localhost:15500")
            //     .WithAuthToken("your-api-key");
            // using var apiKeyClient = new SynapClient(apiKeyConfig);
            // var value = await apiKeyClient.KV.GetAsync("test:switch");
            // if (value != null)
            // {
            //     var valueStr = System.Text.Encoding.UTF8.GetString(value);
            //     Console.WriteLine($"✅ Retrieved value using API Key: {valueStr}");
            // }

            // Clean up
            await basicClient.KV.DeleteAsync("test:switch");
            Console.WriteLine("✅ Cleaned up test key\n");
        }
    }
}

