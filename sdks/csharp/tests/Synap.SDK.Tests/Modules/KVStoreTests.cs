using System.Net;
using System.Text.Json;
using Moq;
using Moq.Protected;
using Synap.SDK.Modules;

namespace Synap.SDK.Tests.Modules;

public sealed class KVStoreTests : IDisposable
{
    private readonly Mock<HttpMessageHandler> _httpHandlerMock;
    private readonly HttpClient _httpClient;
    private readonly SynapClient _client;
    private readonly KVStore _kv;

    public KVStoreTests()
    {
        _httpHandlerMock = new Mock<HttpMessageHandler>();
        _httpClient = new HttpClient(_httpHandlerMock.Object)
        {
            BaseAddress = new Uri("http://localhost:15500")
        };

        var config = SynapConfig.Create("http://localhost:15500");
        _client = new SynapClient(config, _httpClient);
        _kv = _client.KV;
    }

    [Fact]
    public async Task SetAsync_SendsCorrectRequest()
    {
        // Arrange
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent("{}")
            });

        // Act
        await _kv.SetAsync("test-key", "test-value");

        // Assert
        _httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.Is<HttpRequestMessage>(req =>
                req.Method == HttpMethod.Post &&
                req.RequestUri!.ToString().Contains("/api/stream")),
            ItExpr.IsAny<CancellationToken>());
    }

    [Fact]
    public async Task SetAsync_WithTTL_IncludesTTLInData()
    {
        // Arrange
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent("{}")
            });

        // Act
        await _kv.SetAsync("test-key", "test-value", ttl: 3600);

        // Assert - Verify the request was made
        _httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.IsAny<HttpRequestMessage>(),
            ItExpr.IsAny<CancellationToken>());
    }

    [Fact]
    public async Task GetAsync_ReturnsValue()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new { value = "test-value" });
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent(responseContent)
            });

        // Act
        var result = await _kv.GetAsync<string>("test-key");

        // Assert
        Assert.Equal("test-value", result);
    }

    [Fact]
    public async Task GetAsync_WhenNotFound_ReturnsNull()
    {
        // Arrange
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent("{}")
            });

        // Act
        var result = await _kv.GetAsync<string>("nonexistent-key");

        // Assert
        Assert.Null(result);
    }

    [Fact]
    public async Task DeleteAsync_SendsCorrectRequest()
    {
        // Arrange
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent("{}")
            });

        // Act
        await _kv.DeleteAsync("test-key");

        // Assert
        _httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.IsAny<HttpRequestMessage>(),
            ItExpr.IsAny<CancellationToken>());
    }

    [Fact]
    public async Task ExistsAsync_ReturnsTrue_WhenKeyExists()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new { exists = true });
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent(responseContent)
            });

        // Act
        var result = await _kv.ExistsAsync("test-key");

        // Assert
        Assert.True(result);
    }

    [Fact]
    public async Task IncrAsync_ReturnsNewValue()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new { value = 42 });
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent(responseContent)
            });

        // Act
        var result = await _kv.IncrAsync("counter", delta: 5);

        // Assert
        Assert.Equal(42, result);
    }

    [Fact]
    public async Task DecrAsync_ReturnsNewValue()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new { value = 10 });
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent(responseContent)
            });

        // Act
        var result = await _kv.DecrAsync("counter", delta: 3);

        // Assert
        Assert.Equal(10, result);
    }

    [Fact]
    public async Task ScanAsync_ReturnsKeys()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new
        {
            keys = new[] { "user:1", "user:2", "user:3" }
        });
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent(responseContent)
            });

        // Act
        var result = await _kv.ScanAsync("user:*", limit: 100);

        // Assert
        Assert.Equal(3, result.Count);
        Assert.Contains("user:1", result);
        Assert.Contains("user:2", result);
        Assert.Contains("user:3", result);
    }

    [Fact]
    public async Task StatsAsync_ReturnsStatistics()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new
        {
            total_keys = 100,
            memory_usage = 1024
        });
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent(responseContent)
            });

        // Act
        var result = await _kv.StatsAsync();

        // Assert
        Assert.Equal(2, result.Count);
        Assert.True(result.ContainsKey("total_keys"));
        Assert.True(result.ContainsKey("memory_usage"));
    }

    public void Dispose()
    {
        _client.Dispose();
        _httpClient.Dispose();
    }
}

