using System.Diagnostics;
using Synap.SDK;
using Synap.SDK.Modules;
using Xunit;

namespace Synap.SDK.Tests.Modules;

/// <summary>
/// Server-to-Server (S2S) integration tests for HyperLogLogManager.
/// These tests require a running Synap server.
/// Set SYNAP_URL environment variable to point to the server (default: http://localhost:15500).
/// </summary>
public sealed class HyperLogLogManagerS2STests : IDisposable
{
    private readonly SynapClient _client;

    public HyperLogLogManagerS2STests()
    {
        var url = Environment.GetEnvironmentVariable("SYNAP_URL") ?? "http://localhost:15500";
        var config = SynapConfig.Create(url);
        _client = new SynapClient(config);
    }

    [Fact]
    public async Task PfAdd_PfCount_Works()
    {
        var key = $"test:hll:{Process.GetCurrentProcess().Id}";

        var added = await _client.HyperLogLog.PfAddAsync(key, new[] { "user:1", "user:2", "user:3" });
        Assert.True(added >= 0 && added <= 3);

        var count = await _client.HyperLogLog.PfCountAsync(key);
        // Approximate, may be slightly off
        Assert.True(count >= 2 && count <= 4);
    }

    [Fact]
    public async Task PfMerge_Works()
    {
        var timestamp = Process.GetCurrentProcess().Id;
        var key1 = $"test:hll:merge1:{timestamp}";
        var key2 = $"test:hll:merge2:{timestamp}";
        var dest = $"test:hll:merge_dest:{timestamp}";

        await _client.HyperLogLog.PfAddAsync(key1, new[] { "user:1", "user:2", "user:3" });
        await _client.HyperLogLog.PfAddAsync(key2, new[] { "user:4", "user:5", "user:6" });

        var count = await _client.HyperLogLog.PfMergeAsync(dest, new[] { key1, key2 });
        // Approximate
        Assert.True(count >= 5 && count <= 7);
    }

    [Fact]
    public async Task Stats_ReturnsValidData()
    {
        var key = $"test:hll:stats:{Process.GetCurrentProcess().Id}";

        await _client.HyperLogLog.PfAddAsync(key, new[] { "user:1", "user:2" });
        await _client.HyperLogLog.PfCountAsync(key);

        var stats = await _client.HyperLogLog.StatsAsync();
        Assert.True(stats.PfAddCount >= 1);
        Assert.True(stats.PfCountCount >= 1);
    }

    public void Dispose()
    {
        _client?.Dispose();
    }
}

