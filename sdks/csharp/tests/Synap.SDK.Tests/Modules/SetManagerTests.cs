using System.Collections.Generic;
using System.Net;
using System.Net.Http;
using System.Text;
using Synap.SDK.Modules;
using System.Text.Json;
using Xunit;

namespace Synap.SDK.Tests.Modules;

public sealed class SetManagerTests
{
    [Fact]
    public async Task AddAsync_ShouldReturnAddedCount()
    {
        using var testClient = CreateClient(new { added = 3 });
        var manager = new SetManager(testClient.Client);

        var result = await manager.AddAsync("tags", new List<string> { "python", "redis" });

        Assert.Equal(3, result);
    }

    [Fact]
    public async Task RemAsync_ShouldReturnRemovedCount()
    {
        using var testClient = CreateClient(new { removed = 1 });
        var manager = new SetManager(testClient.Client);

        var result = await manager.RemAsync("tags", new List<string> { "redis" });

        Assert.Equal(1, result);
    }

    [Fact]
    public async Task IsMemberAsync_ShouldReturnTrue()
    {
        using var testClient = CreateClient(new { is_member = true });
        var manager = new SetManager(testClient.Client);

        var result = await manager.IsMemberAsync("tags", "python");

        Assert.True(result);
    }

    [Fact]
    public async Task MembersAsync_ShouldReturnMembers()
    {
        using var testClient = CreateClient(new { members = new[] { "python", "redis" } });
        var manager = new SetManager(testClient.Client);

        var result = await manager.MembersAsync("tags");

        Assert.Equal(2, result.Count);
    }

    [Fact]
    public async Task CardAsync_ShouldReturnCardinality()
    {
        using var testClient = CreateClient(new { cardinality = 3 });
        var manager = new SetManager(testClient.Client);

        var result = await manager.CardAsync("tags");

        Assert.Equal(3, result);
    }

    [Fact]
    public async Task InterAsync_ShouldReturnIntersection()
    {
        using var testClient = CreateClient(new { members = new[] { "python" } });
        var manager = new SetManager(testClient.Client);

        var result = await manager.InterAsync(new List<string> { "tags1", "tags2" });

        Assert.Single(result);
        Assert.Equal("python", result[0]);
    }

    [Fact]
    public async Task UnionAsync_ShouldReturnUnion()
    {
        using var testClient = CreateClient(new { members = new[] { "python", "redis", "typescript" } });
        var manager = new SetManager(testClient.Client);

        var result = await manager.UnionAsync(new List<string> { "tags1", "tags2" });

        Assert.Equal(3, result.Count);
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

