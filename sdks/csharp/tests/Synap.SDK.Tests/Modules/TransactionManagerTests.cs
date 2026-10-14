using System.Net;
using Moq;
using Moq.Protected;
using Synap.SDK.Modules;

namespace Synap.SDK.Tests.Modules;

/// <summary>
/// TransactionManager over HTTP, against the command envelope the server
/// really sends. The client unwraps <c>payload</c>, so the manager must read
/// the fields from the unwrapped document.
/// </summary>
public sealed class TransactionManagerTests : IDisposable
{
    private readonly Mock<HttpMessageHandler> _httpHandlerMock = new();
    private readonly HttpClient _httpClient;
    private readonly SynapClient _client;

    public TransactionManagerTests()
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
    public async Task MultiDiscardWatchUnwatch_ReadTheServerResponse()
    {
        RespondWithPayload("{\"message\":\"Transaction started\",\"success\":true}");
        var multi = await _client.Transaction.MultiAsync("c1");
        Assert.True(multi.Success);
        Assert.Equal("Transaction started", multi.Message);

        RespondWithPayload("{\"message\":\"Keys watched\",\"success\":true}");
        Assert.Equal("Keys watched", (await _client.Transaction.WatchAsync(["k"], "c1")).Message);

        RespondWithPayload("{\"message\":\"Keys unwatched\",\"success\":true}");
        Assert.Equal("Keys unwatched", (await _client.Transaction.UnwatchAsync("c1")).Message);

        RespondWithPayload("{\"message\":\"Transaction discarded\",\"success\":true}");
        Assert.Equal("Transaction discarded", (await _client.Transaction.DiscardAsync("c1")).Message);
    }

    [Fact]
    public async Task ExecAsync_ReturnsTheResultsAndTheyOutliveTheResponse()
    {
        RespondWithPayload("{\"results\":[{\"ok\":true},{\"ok\":true}],\"success\":true}");
        var exec = Assert.IsType<TransactionExecSuccess>(await _client.Transaction.ExecAsync("c1"));
        Assert.Equal(2, exec.Results.Count);
        // The response document is disposed when ExecAsync returns; the
        // results must still be readable.
        Assert.True(exec.Results[0].GetProperty("ok").GetBoolean());
    }

    [Fact]
    public async Task ExecAsync_WithNoQueuedCommands_IsASuccessWithNoResults()
    {
        RespondWithPayload("{\"results\":[],\"success\":true}");
        var exec = Assert.IsType<TransactionExecSuccess>(await _client.Transaction.ExecAsync("c1"));
        Assert.Empty(exec.Results);
    }

    [Fact]
    public async Task ExecAsync_WhenAWatchedKeyChanged_ReturnsAborted()
    {
        RespondWithPayload("{\"aborted\":true,\"message\":\"Transaction aborted: watched keys changed\"}");
        var aborted = Assert.IsType<TransactionExecAborted>(await _client.Transaction.ExecAsync("c1"));
        Assert.True(aborted.Aborted);
        Assert.False(aborted.Success);
        Assert.Equal("Transaction aborted: watched keys changed", aborted.Message);
    }

    public void Dispose()
    {
        _client.Dispose();
        _httpClient.Dispose();
    }
}
