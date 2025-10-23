using System.Net;
using System.Text.Json;
using Moq;
using Moq.Protected;
using Synap.SDK.Modules;

namespace Synap.SDK.Tests.Modules;

public sealed class StreamManagerTests : IDisposable
{
    private readonly Mock<HttpMessageHandler> _httpHandlerMock;
    private readonly HttpClient _httpClient;
    private readonly SynapClient _client;
    private readonly StreamManager _stream;

    public StreamManagerTests()
    {
        _httpHandlerMock = new Mock<HttpMessageHandler>();
        _httpClient = new HttpClient(_httpHandlerMock.Object)
        {
            BaseAddress = new Uri("http://localhost:15500")
        };

        var config = SynapConfig.Create("http://localhost:15500");
        _client = new SynapClient(config, _httpClient);
        _stream = _client.Stream;
    }

    [Fact]
    public async Task CreateRoomAsync_SendsCorrectRequest()
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
        await _stream.CreateRoomAsync("test-room");

        // Assert
        _httpHandlerMock.Protected().Verify(
            "SendAsync",
            Times.Once(),
            ItExpr.IsAny<HttpRequestMessage>(),
            ItExpr.IsAny<CancellationToken>());
    }

    [Fact]
    public async Task PublishAsync_ReturnsOffset()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new { offset = 42L });
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
        var offset = await _stream.PublishAsync("test-room", "user.created", new { userId = "123" });

        // Assert
        Assert.Equal(42L, offset);
    }

    [Fact]
    public async Task ReadAsync_ReturnsEvents()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new
        {
            events = new[]
            {
                new
                {
                    offset = 0L,
                    @event = "user.created",
                    data = JsonSerializer.SerializeToElement(new { userId = "123" }),
                    timestamp = 1234567890L,
                    room = "test-room"
                },
                new
                {
                    offset = 1L,
                    @event = "user.updated",
                    data = JsonSerializer.SerializeToElement(new { userId = "123", name = "Alice" }),
                    timestamp = 1234567891L,
                    room = "test-room"
                }
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
        var events = await _stream.ReadAsync("test-room", offset: 0, limit: 10);

        // Assert
        Assert.Equal(2, events.Count);
        Assert.Equal("user.created", events[0].Event);
        Assert.Equal(0L, events[0].Offset);
        Assert.Equal("user.updated", events[1].Event);
        Assert.Equal(1L, events[1].Offset);
    }

    [Fact]
    public async Task ListRoomsAsync_ReturnsRooms()
    {
        // Arrange
        var responseContent = JsonSerializer.Serialize(new
        {
            rooms = new[] { "room1", "room2", "room3" }
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
        var rooms = await _stream.ListRoomsAsync();

        // Assert
        Assert.Equal(3, rooms.Count);
        Assert.Contains("room1", rooms);
    }

    public void Dispose()
    {
        _client.Dispose();
        _httpClient.Dispose();
    }
}

