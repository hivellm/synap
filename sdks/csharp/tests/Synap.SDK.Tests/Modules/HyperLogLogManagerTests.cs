using System.Net;
using Moq;
using Moq.Protected;

namespace Synap.SDK.Tests.Modules;

/// <summary>
/// HyperLogLogManager over HTTP, against the command envelope the server
/// really sends. The client unwraps <c>payload</c>, so the manager must read
/// the fields from the unwrapped document.
/// </summary>
public sealed class HyperLogLogManagerTests : IDisposable
{
    private readonly Mock<HttpMessageHandler> _httpHandlerMock = new();
    private readonly HttpClient _httpClient;
    private readonly SynapClient _client;

    public HyperLogLogManagerTests()
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
    public async Task PfAddAsync_ReturnsTheAddedCountFromTheEnvelope()
    {
        RespondWithPayload("{\"added\":3,\"key\":\"h\"}");
        Assert.Equal(3, await _client.HyperLogLog.PfAddAsync("h", ["a", "b", "c"]));
    }

    [Fact]
    public async Task PfCountAsync_ReturnsTheCountFromTheEnvelope()
    {
        RespondWithPayload("{\"count\":3,\"key\":\"h\"}");
        Assert.Equal(3, await _client.HyperLogLog.PfCountAsync("h"));
    }

    [Fact]
    public async Task PfMergeAsync_ReturnsTheMergedCountFromTheEnvelope()
    {
        RespondWithPayload("{\"count\":4,\"destination\":\"d\"}");
        Assert.Equal(4, await _client.HyperLogLog.PfMergeAsync("d", ["h1", "h2"]));
    }

    [Fact]
    public async Task StatsAsync_ReadsTheStatisticsFromTheEnvelope()
    {
        RespondWithPayload("{\"pfadd_count\":6,\"pfcount_count\":3,\"pfmerge_count\":2,"
            + "\"total_cardinality\":267,\"total_hlls\":52}");
        var stats = await _client.HyperLogLog.StatsAsync();
        Assert.Equal(52, stats.TotalHlls);
        Assert.Equal(267, stats.TotalCardinality);
        Assert.Equal(6, stats.PfAddCount);
        Assert.Equal(3, stats.PfCountCount);
        Assert.Equal(2, stats.PfMergeCount);
    }

    public void Dispose()
    {
        _client.Dispose();
        _httpClient.Dispose();
    }
}
