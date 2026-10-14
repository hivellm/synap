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

    // The tests below answer with the envelope the server really sends:
    // {"success":true,"request_id":...,"payload":{...},"error":null}.

    private void RespondWithPayload(string payloadJson, Action<string>? captureRequest = null)
    {
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .Returns(async (HttpRequestMessage request, CancellationToken _) =>
            {
                captureRequest?.Invoke(await request.Content!.ReadAsStringAsync());
                return new HttpResponseMessage
                {
                    StatusCode = HttpStatusCode.OK,
                    Content = new StringContent(
                        $"{{\"success\":true,\"request_id\":\"r1\",\"payload\":{payloadJson},\"error\":null}}"),
                };
            });
    }

    private static string ByteList(string text) =>
        "[" + string.Join(",", System.Text.Encoding.UTF8.GetBytes(text)) + "]";

    [Fact]
    public async Task PublishAsync_SendsThePayloadAsTheByteListTheServerReads()
    {
        // The server reads `payload` as an array of bytes and refuses anything
        // else with "Missing 'payload' field".
        string? body = null;
        RespondWithPayload("{\"message_id\":\"m-1\"}", b => body = b);

        var id = await _queue.PublishAsync("q", new { data = "hello" }, priority: 5);

        Assert.Equal("m-1", id);
        using var sent = JsonDocument.Parse(body!);
        var payload = sent.RootElement.GetProperty("payload").GetProperty("payload");
        var bytes = payload.EnumerateArray().Select(b => b.GetByte()).ToArray();
        Assert.Equal("{\"data\":\"hello\"}", System.Text.Encoding.UTF8.GetString(bytes));
    }

    [Fact]
    public async Task PublishAsync_SendsAStringAsItsText()
    {
        string? body = null;
        RespondWithPayload("{\"message_id\":\"m-2\"}", b => body = b);

        await _queue.PublishAsync("q", "plain text");

        using var sent = JsonDocument.Parse(body!);
        var bytes = sent.RootElement.GetProperty("payload").GetProperty("payload")
            .EnumerateArray().Select(b => b.GetByte()).ToArray();
        Assert.Equal("plain text", System.Text.Encoding.UTF8.GetString(bytes));
    }

    [Fact]
    public async Task ConsumeAsync_DecodesTheByteListAndTheMessageOutlivesTheResponse()
    {
        RespondWithPayload("{\"message\":{\"headers\":{},\"id\":\"m-1\",\"max_retries\":3,"
            + $"\"payload\":{ByteList("{\"data\":\"hello\"}")},\"priority\":7,\"retry_count\":2}}}}");

        var message = await _queue.ConsumeAsync("q", "w");

        Assert.NotNull(message);
        Assert.Equal("m-1", message.Id);
        Assert.Equal(7, message.Priority);
        Assert.Equal(2, message.Retries);
        // The response document is disposed when ConsumeAsync returns.
        Assert.Equal("hello", message.Payload!.Value.GetProperty("data").GetString());
    }

    [Fact]
    public async Task ConsumeAsync_GivesTextThatIsNotJsonAsAString()
    {
        RespondWithPayload($"{{\"message\":{{\"id\":\"m-1\",\"payload\":{ByteList("plain text")},\"priority\":5,\"retry_count\":0}}}}");

        var message = await _queue.ConsumeAsync("q", "w");

        Assert.Equal("plain text", message!.Payload!.Value.GetString());
    }

    [Fact]
    public async Task ConsumeAsync_KeepsBytesThatAreNotUtf8AsTheByteList()
    {
        RespondWithPayload("{\"message\":{\"id\":\"m-1\",\"payload\":[222,173,190,239],\"priority\":5,\"retry_count\":0}}");

        var message = await _queue.ConsumeAsync("q", "w");

        Assert.Equal(new byte[] { 0xDE, 0xAD, 0xBE, 0xEF },
            message!.Payload!.Value.EnumerateArray().Select(b => b.GetByte()).ToArray());
    }

    [Fact]
    public async Task ConsumeAsync_OnAnEmptyQueue_ReturnsNull()
    {
        // The server's empty answer is {"message": null}, not a missing field.
        RespondWithPayload("{\"message\":null}");

        Assert.Null(await _queue.ConsumeAsync("q", "w"));
    }

    public void Dispose()
    {
        _client.Dispose();
        _httpClient.Dispose();
    }
}

