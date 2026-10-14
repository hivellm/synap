using System.Net;
using Moq;
using Moq.Protected;
using Synap.SDK.Modules;

namespace Synap.SDK.Tests.Modules;

/// <summary>
/// BitmapManager over HTTP, against the command envelope the server really
/// sends: <c>{"success":true,"request_id":...,"payload":{...},"error":null}</c>.
/// The client unwraps <c>payload</c>, so the manager must read the fields from
/// the unwrapped document.
/// </summary>
public sealed class BitmapManagerTests : IDisposable
{
    private readonly Mock<HttpMessageHandler> _httpHandlerMock = new();
    private readonly HttpClient _httpClient;
    private readonly SynapClient _client;

    public BitmapManagerTests()
    {
        _httpClient = new HttpClient(_httpHandlerMock.Object)
        {
            BaseAddress = new Uri("http://localhost:15500"),
        };
        _client = new SynapClient(SynapConfig.Create("http://localhost:15500"), _httpClient);
    }

    private void RespondWithPayload(string payloadJson)
    {
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(() => new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent(
                    $"{{\"success\":true,\"request_id\":\"r1\",\"payload\":{payloadJson},\"error\":null}}"),
            });
    }

    [Fact]
    public async Task SetBitAsync_ReturnsTheOldValueFromTheEnvelope()
    {
        RespondWithPayload("{\"key\":\"bm\",\"offset\":5,\"old_value\":1}");
        Assert.Equal(1, await _client.Bitmap.SetBitAsync("bm", 5, 0));
    }

    [Fact]
    public async Task GetBitAsync_ReturnsTheValueFromTheEnvelope()
    {
        RespondWithPayload("{\"key\":\"bm\",\"offset\":5,\"value\":1}");
        Assert.Equal(1, await _client.Bitmap.GetBitAsync("bm", 5));
    }

    [Fact]
    public async Task BitCountAsync_ReturnsTheCountFromTheEnvelope()
    {
        RespondWithPayload("{\"count\":4,\"key\":\"bm\"}");
        Assert.Equal(4, await _client.Bitmap.BitCountAsync("bm"));
    }

    [Fact]
    public async Task BitPosAsync_ReturnsThePositionOrNull()
    {
        RespondWithPayload("{\"key\":\"bm\",\"position\":7}");
        Assert.Equal(7, await _client.Bitmap.BitPosAsync("bm", 1));

        RespondWithPayload("{\"key\":\"bm\",\"position\":null}");
        Assert.Null(await _client.Bitmap.BitPosAsync("bm", 1));
    }

    [Fact]
    public async Task BitOpAsync_ReturnsTheLengthFromTheEnvelope()
    {
        RespondWithPayload("{\"destination\":\"dst\",\"length\":16}");
        Assert.Equal(16, await _client.Bitmap.BitOpAsync(BitmapManager.BitmapOperation.AND, "dst", ["a", "b"]));
    }

    [Fact]
    public async Task BitFieldAsync_ReturnsTheResultsFromTheEnvelope()
    {
        RespondWithPayload("{\"key\":\"bf\",\"results\":[0,42]}");
        var results = await _client.Bitmap.BitFieldAsync("bf",
        [
            new BitfieldOperation { Operation = BitmapManager.BitfieldOperationType.SET, Offset = 0, Width = 8, Value = 42 },
            new BitfieldOperation { Operation = BitmapManager.BitfieldOperationType.GET, Offset = 0, Width = 8 },
        ]);
        Assert.Equal(new List<long> { 0, 42 }, results);
    }

    [Fact]
    public async Task StatsAsync_ReadsTheStatisticsFromTheEnvelope()
    {
        RespondWithPayload("{\"bitcount_count\":4,\"bitfield_count\":9,\"bitop_count\":1,\"bitpos_count\":3,"
            + "\"getbit_count\":2,\"setbit_count\":17,\"total_bitmaps\":5,\"total_bits\":1000}");
        var stats = await _client.Bitmap.StatsAsync();
        Assert.Equal(5, stats.TotalBitmaps);
        Assert.Equal(1000, stats.TotalBits);
        Assert.Equal(17, stats.SetBitCount);
        Assert.Equal(9, stats.BitFieldCount);
    }

    public void Dispose()
    {
        _client.Dispose();
        _httpClient.Dispose();
    }
}
