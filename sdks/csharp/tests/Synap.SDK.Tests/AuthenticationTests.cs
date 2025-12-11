using System;
using System.Net.Http;
using System.Threading.Tasks;
using Moq;
using Moq.Protected;
using Synap.SDK;
using Synap.SDK.Exceptions;
using Xunit;

namespace Synap.SDK.Tests;

public sealed class AuthenticationTests : IDisposable
{
    private const string TestUrl = "http://localhost:15500";
    private const string TestUsername = "root";
    private const string TestPassword = "root";

    [Fact]
    public void BasicAuthConfigCreation_WithValidCredentials_SetsProperties()
    {
        // Arrange & Act
        var config = SynapConfig.Create(TestUrl)
            .WithBasicAuth(TestUsername, TestPassword);

        // Assert
        Assert.Equal(TestUsername, config.Username);
        Assert.Equal(TestPassword, config.Password);
        Assert.Null(config.AuthToken);
    }

    [Fact]
    public void ApiKeyConfigCreation_WithValidToken_SetsProperty()
    {
        // Arrange & Act
        var config = SynapConfig.Create(TestUrl)
            .WithAuthToken("sk_test123");

        // Assert
        Assert.Equal("sk_test123", config.AuthToken);
        Assert.Null(config.Username);
        Assert.Null(config.Password);
    }

    [Fact]
    public void ConfigBuilderPattern_WithMultipleCalls_AppliesAll()
    {
        // Arrange & Act
        var config = SynapConfig.Create(TestUrl)
            .WithTimeout(60)
            .WithBasicAuth("user", "pass");

        // Assert
        Assert.Equal(60, config.Timeout);
        Assert.Equal("user", config.Username);
        Assert.Equal("pass", config.Password);
    }

    [Fact]
    public void AuthTokenOverridesBasicAuth_WhenCalledAfterBasicAuth()
    {
        // Arrange & Act
        var config = SynapConfig.Create(TestUrl)
            .WithBasicAuth("user", "pass")
            .WithAuthToken("sk_test123");

        // Assert
        Assert.Equal("sk_test123", config.AuthToken);
        Assert.Null(config.Username);
        Assert.Null(config.Password);
    }

    [Fact]
    public void BasicAuthOverridesAuthToken_WhenCalledAfterAuthToken()
    {
        // Arrange & Act
        var config = SynapConfig.Create(TestUrl)
            .WithAuthToken("sk_test123")
            .WithBasicAuth("user", "pass");

        // Assert
        Assert.Equal("user", config.Username);
        Assert.Equal("pass", config.Password);
        Assert.Null(config.AuthToken);
    }

    [Fact]
    public void ClientWithBasicAuth_CreatesClient()
    {
        // Arrange
        var config = SynapConfig.Create(TestUrl)
            .WithBasicAuth(TestUsername, TestPassword);

        // Act
        using var client = new SynapClient(config);

        // Assert
        Assert.NotNull(client);
    }

    [Fact]
    public void ClientWithApiKey_CreatesClient()
    {
        // Arrange
        var config = SynapConfig.Create(TestUrl)
            .WithAuthToken("sk_test123");

        // Act
        using var client = new SynapClient(config);

        // Assert
        Assert.NotNull(client);
    }

    [Fact]
    public void ClientWithoutAuth_CreatesClient()
    {
        // Arrange
        var config = SynapConfig.Create(TestUrl);

        // Act
        using var client = new SynapClient(config);

        // Assert
        Assert.NotNull(client);
    }

    [Fact]
    public async Task ClientWithBasicAuth_SendsAuthorizationHeader()
    {
        // Arrange
        var httpHandlerMock = new Mock<HttpMessageHandler>();
        var httpClient = new HttpClient(httpHandlerMock.Object)
        {
            BaseAddress = new Uri(TestUrl)
        };

        var config = SynapConfig.Create(TestUrl)
            .WithBasicAuth(TestUsername, TestPassword);
        using var client = new SynapClient(config, httpClient);

        httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.Is<HttpRequestMessage>(req =>
                    req.Headers.Authorization != null &&
                    req.Headers.Authorization.Scheme == "Basic"),
                ItExpr.IsAny<System.Threading.CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = System.Net.HttpStatusCode.OK,
                Content = new StringContent("{\"success\": true, \"payload\": {}}")
            });

        // Act
        await client.KV.SetAsync("test-key", System.Text.Encoding.UTF8.GetBytes("test-value"));

        // Assert
        httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.Is<HttpRequestMessage>(req =>
                req.Headers.Authorization != null &&
                req.Headers.Authorization.Scheme == "Basic"),
            ItExpr.IsAny<System.Threading.CancellationToken>());
    }

    [Fact]
    public async Task ClientWithApiKey_SendsBearerTokenHeader()
    {
        // Arrange
        var httpHandlerMock = new Mock<HttpMessageHandler>();
        var httpClient = new HttpClient(httpHandlerMock.Object)
        {
            BaseAddress = new Uri(TestUrl)
        };

        var config = SynapConfig.Create(TestUrl)
            .WithAuthToken("sk_test123");
        using var client = new SynapClient(config, httpClient);

        httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.Is<HttpRequestMessage>(req =>
                    req.Headers.Authorization != null &&
                    req.Headers.Authorization.Scheme == "Bearer" &&
                    req.Headers.Authorization.Parameter == "sk_test123"),
                ItExpr.IsAny<System.Threading.CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = System.Net.HttpStatusCode.OK,
                Content = new StringContent("{\"success\": true, \"payload\": {}}")
            });

        // Act
        await client.KV.SetAsync("test-key", System.Text.Encoding.UTF8.GetBytes("test-value"));

        // Assert
        httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.Is<HttpRequestMessage>(req =>
                req.Headers.Authorization != null &&
                req.Headers.Authorization.Scheme == "Bearer" &&
                req.Headers.Authorization.Parameter == "sk_test123"),
            ItExpr.IsAny<System.Threading.CancellationToken>());
    }

    /// <summary>
    /// S2S Test: Requires running Synap server
    /// Run with: dotnet test --filter "FullyQualifiedName~AuthenticationTests::testBasicAuthS2S" -- SYNAP_URL=http://localhost:15500 SYNAP_TEST_USERNAME=root SYNAP_TEST_PASSWORD=root
    /// </summary>
    [Fact(Skip = "S2S test - requires running Synap server")]
    public async Task BasicAuthS2S_WithValidCredentials_Works()
    {
        var url = Environment.GetEnvironmentVariable("SYNAP_URL") ?? TestUrl;
        var username = Environment.GetEnvironmentVariable("SYNAP_TEST_USERNAME") ?? TestUsername;
        var password = Environment.GetEnvironmentVariable("SYNAP_TEST_PASSWORD") ?? TestPassword;

        var config = SynapConfig.Create(url)
            .WithBasicAuth(username, password);
        using var client = new SynapClient(config);

        // Test KV operation
        await client.KV.SetAsync("auth:test:basic", System.Text.Encoding.UTF8.GetBytes("test_value"));
        var value = await client.KV.GetAsync("auth:test:basic");
        Assert.NotNull(value);
        Assert.Equal("test_value", System.Text.Encoding.UTF8.GetString(value));

        // Cleanup
        await client.KV.DeleteAsync("auth:test:basic");
    }

    /// <summary>
    /// S2S Test: Requires running Synap server with valid API key
    /// Run with: dotnet test --filter "FullyQualifiedName~AuthenticationTests::testApiKeyAuthS2S" -- SYNAP_URL=http://localhost:15500 SYNAP_TEST_API_KEY=sk_...
    /// </summary>
    [Fact(Skip = "S2S test - requires running Synap server with API key")]
    public async Task ApiKeyAuthS2S_WithValidKey_Works()
    {
        var url = Environment.GetEnvironmentVariable("SYNAP_URL") ?? TestUrl;
        var apiKey = Environment.GetEnvironmentVariable("SYNAP_TEST_API_KEY");

        if (string.IsNullOrWhiteSpace(apiKey))
        {
            throw new InvalidOperationException("SYNAP_TEST_API_KEY environment variable required");
        }

        var config = SynapConfig.Create(url)
            .WithAuthToken(apiKey);
        using var client = new SynapClient(config);

        // Test KV operation
        await client.KV.SetAsync("auth:test:apikey", System.Text.Encoding.UTF8.GetBytes("test_value"));
        var value = await client.KV.GetAsync("auth:test:apikey");
        Assert.NotNull(value);
        Assert.Equal("test_value", System.Text.Encoding.UTF8.GetString(value));

        // Cleanup
        await client.KV.DeleteAsync("auth:test:apikey");
    }

    public void Dispose()
    {
        // Cleanup if needed
    }
}

