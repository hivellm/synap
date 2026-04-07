using System.Collections.Generic;
using System.Net;
using System.Net.Http;
using System.Text;
using Synap.SDK.Modules;
using System.Text.Json;
using Xunit;

namespace Synap.SDK.Tests.Modules;

public sealed class ListManagerTests
{
    [Fact]
    public async Task LPushAsync_ShouldReturnLength()
    {
        using var testClient = CreateClient(new { length = 3 });
        var manager = new ListManager(testClient.Client);

        var result = await manager.LPushAsync("tasks", new List<string> { "task1", "task2" });

        Assert.Equal(3, result);
    }

    [Fact]
    public async Task RPushAsync_ShouldReturnLength()
    {
        using var testClient = CreateClient(new { length = 2 });
        var manager = new ListManager(testClient.Client);

        var result = await manager.RPushAsync("tasks", new List<string> { "task1" });

        Assert.Equal(2, result);
    }

    [Fact]
    public async Task LPopAsync_ShouldReturnValues()
    {
        using var testClient = CreateClient(new { values = new[] { "task1" } });
        var manager = new ListManager(testClient.Client);

        var result = await manager.LPopAsync("tasks");

        Assert.Single(result);
        Assert.Equal("task1", result[0]);
    }

    [Fact]
    public async Task RangeAsync_ShouldReturnRange()
    {
        using var testClient = CreateClient(new { values = new[] { "task1", "task2", "task3" } });
        var manager = new ListManager(testClient.Client);

        var result = await manager.RangeAsync("tasks");

        Assert.Equal(3, result.Count);
    }

    [Fact]
    public async Task LenAsync_ShouldReturnLength()
    {
        using var testClient = CreateClient(new { length = 5 });
        var manager = new ListManager(testClient.Client);

        var result = await manager.LenAsync("tasks");

        Assert.Equal(5, result);
    }

    private static TestSynapClient CreateClient(object response)
    {
        var json = response switch
        {
            string s => s,
            JsonDocument doc => doc.RootElement.GetRawText(),
            _ => JsonSerializer.Serialize(response)
        };

        return new TestSynapClient(json);
    }

    private sealed class TestSynapClient : IDisposable
    {
        private readonly HttpClient _httpClient;

        public TestSynapClient(params string[] responses)
        {
            var handler = new StubHttpMessageHandler(responses);
            _httpClient = new HttpClient(handler)
            {
                BaseAddress = new Uri("http://localhost:15500"),
            };

            Client = new SynapClient(new SynapConfig("http://localhost:15500"), _httpClient);
        }

        public SynapClient Client { get; }

        public void Dispose()
        {
            Client.Dispose();
            _httpClient.Dispose();
        }
    }

    private sealed class StubHttpMessageHandler : HttpMessageHandler
    {
        private readonly Queue<string> _responses;

        public StubHttpMessageHandler(IEnumerable<string> responses)
        {
            _responses = new Queue<string>(responses);
        }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            if (_responses.Count == 0)
            {
                throw new InvalidOperationException("No configured response for request.");
            }

            var json = _responses.Dequeue();

            var message = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(json, Encoding.UTF8, "application/json"),
            };

            return Task.FromResult(message);
        }
    }
}

