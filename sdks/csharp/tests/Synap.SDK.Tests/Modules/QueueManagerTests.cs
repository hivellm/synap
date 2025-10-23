using System.Net;
using System.Text.Json;
using Moq;
using Moq.Protected;
using Synap.SDK.Modules;

namespace Synap.SDK.Tests.Modules;

public sealed class QueueManagerTests : IDisposable
{
    private readonly Mock<HttpMessageHandler> _httpHandlerMock;
    private readonly HttpClient _httpClient;
    private readonly SynapClient _client;
    private readonly QueueManager _queue;

    public QueueManagerTests()
    {
        _httpHandlerMock = new Mock<HttpMessageHandler>();
        _httpClient = new HttpClient(_httpHandlerMock.Object)
        {
            BaseAddress = new Uri("http://localhost:15500")
        };

        var config = SynapConfig.Create("http://localhost:15500");
        _client = new SynapClient(config, _httpClient);
        _queue = _client.Queue;
    }

    [Fact]
    public async Task CreateQueueAsync_SendsCorrectRequest()
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
        await _queue.CreateQueueAsync("test-queue", maxSize: 1000, messageTtl: 3600);

        // Assert
        _httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.IsAny<HttpRequestMessage>(),
            ItExpr.IsAny<CancellationToken>());
    }

    [Fact]
    public async Task PublishAsync_ReturnsMessageId()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new { message_id = "msg-123" });
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
        var messageId = await _queue.PublishAsync("test-queue", new { data = "test" }, priority: 9);

        // Assert
        Assert.Equal("msg-123", messageId);
    }

    [Fact]
    public async Task ConsumeAsync_ReturnsMessage()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new
        {
            message = new
            {
                id = "msg-456",
                payload = JsonSerializer.SerializeToElement(new { data = "test" }),
                priority = 5,
                retries = 0,
                max_retries = 3,
                timestamp = 1234567890L
            }
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
        var message = await _queue.ConsumeAsync("test-queue", "worker-1");

        // Assert
        Assert.NotNull(message);
        Assert.Equal("msg-456", message.Id);
        Assert.Equal(5, message.Priority);
        Assert.Equal(0, message.Retries);
        Assert.Equal(3, message.MaxRetries);
    }

    [Fact]
    public async Task ConsumeAsync_WhenNoMessage_ReturnsNull()
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
        var message = await _queue.ConsumeAsync("test-queue", "worker-1");

        // Assert
        Assert.Null(message);
    }

    [Fact]
    public async Task AckAsync_SendsCorrectRequest()
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
        await _queue.AckAsync("test-queue", "msg-123");

        // Assert
        _httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.IsAny<HttpRequestMessage>(),
            ItExpr.IsAny<CancellationToken>());
    }

    [Fact]
    public async Task ListAsync_ReturnsQueues()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new
        {
            queues = new[] { "queue1", "queue2", "queue3" }
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
        var queues = await _queue.ListAsync();

        // Assert
        Assert.Equal(3, queues.Count);
        Assert.Contains("queue1", queues);
        Assert.Contains("queue2", queues);
    }

    public void Dispose()
    {
        _client.Dispose();
        _httpClient.Dispose();
    }
}

