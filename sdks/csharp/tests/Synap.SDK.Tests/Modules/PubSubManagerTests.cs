using System.Net;
using System.Text.Json;
using Moq;
using Moq.Protected;
using Synap.SDK.Modules;

namespace Synap.SDK.Tests.Modules;

public sealed class PubSubManagerTests : IDisposable
{
    private readonly Mock<HttpMessageHandler> _httpHandlerMock;
    private readonly HttpClient _httpClient;
    private readonly SynapClient _client;
    private readonly PubSubManager _pubsub;

    public PubSubManagerTests()
    {
        _httpHandlerMock = new Mock<HttpMessageHandler>();
        _httpClient = new HttpClient(_httpHandlerMock.Object)
        {
            BaseAddress = new Uri("http://localhost:15500")
        };

        var config = SynapConfig.Create("http://localhost:15500");
        _client = new SynapClient(config, _httpClient);
        _pubsub = _client.PubSub;
    }

    [Fact]
    public async Task SubscribeTopicsAsync_SendsCorrectRequest()
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
        await _pubsub.SubscribeTopicsAsync("subscriber-1", new List<string> { "notifications.*", "alerts.#" });

        // Assert
        _httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.IsAny<HttpRequestMessage>(),
            ItExpr.IsAny<CancellationToken>());
    }

    [Fact]
    public async Task UnsubscribeTopicsAsync_SendsCorrectRequest()
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
        await _pubsub.UnsubscribeTopicsAsync("subscriber-1", new List<string> { "notifications.*" });

        // Assert
        _httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.IsAny<HttpRequestMessage>(),
            ItExpr.IsAny<CancellationToken>());
    }

    [Fact]
    public async Task PublishAsync_ReturnsDeliveredCount()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new { subscribers_matched = 5 });
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
        var delivered = await _pubsub.PublishAsync("notifications.email", new { to = "user@example.com" });

        // Assert
        Assert.Equal(5, delivered);
    }

    [Fact]
    public async Task StatsAsync_ReturnsStatistics()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new
        {
            total_subscribers = 10,
            total_topics = 25
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
        var stats = await _pubsub.StatsAsync();

        // Assert
        Assert.Equal(2, stats.Count);
        Assert.True(stats.ContainsKey("total_subscribers"));
        Assert.True(stats.ContainsKey("total_topics"));
    }

    public void Dispose()
    {
        _client.Dispose();
        _httpClient.Dispose();
    }
}

