using System.Threading;
using System.Threading.Tasks;
using Synap.SDK;
using Synap.SDK.Exceptions;
using Synap.SDK.Types;
using Xunit;

namespace Synap.SDK.Tests;

public sealed class WatchEventTests
{
    [Fact]
    public void FromJson_FullEnvelope_Decodes()
    {
        var envelope = WatchEvent.FromJson(
            "{\"key\":\"user:1\",\"event\":\"set\",\"version\":3,\"value\":\"alice\"}");

        Assert.NotNull(envelope);
        Assert.Equal("user:1", envelope.Key);
        Assert.Equal("set", envelope.Event);
        Assert.Equal(3, envelope.Version);
        Assert.Equal("alice", envelope.Value);
        Assert.False(envelope.Truncated);
    }

    [Fact]
    public void FromJson_OmittedOptionalFields_TakeDefaults()
    {
        // A del envelope omits value and truncated entirely.
        var envelope = WatchEvent.FromJson("{\"key\":\"k\",\"event\":\"del\",\"version\":7}");

        Assert.NotNull(envelope);
        Assert.Null(envelope.Value);
        Assert.False(envelope.Truncated);
        Assert.Equal(7, envelope.Version);
    }

    [Fact]
    public void FromJson_TruncatedEnvelope_KeepsTheFlag()
    {
        var envelope = WatchEvent.FromJson(
            "{\"key\":\"big\",\"event\":\"set\",\"version\":1,\"truncated\":true}");

        Assert.NotNull(envelope);
        Assert.True(envelope.Truncated);
        Assert.Null(envelope.Value);
    }

    [Fact]
    public void FromJson_NonEnvelopePayload_ReturnsNull()
    {
        Assert.Null(WatchEvent.FromJson("not json"));
        Assert.Null(WatchEvent.FromJson("{\"unrelated\":true}"));
    }

    [Fact]
    public async Task WatchAsync_OnHttpTransport_Throws()
    {
        // An http:// client has no SynapRPC transport.
        var config = SynapConfig.Create("http://localhost:15500");
        using var client = new SynapClient(config);

        var exception = await Assert.ThrowsAsync<SynapException>(async () =>
        {
            await foreach (var _ in client.KV.WatchAsync("k", cancellationToken: CancellationToken.None))
            {
                break;
            }
        });

        Assert.Contains("synap://", exception.Message, System.StringComparison.Ordinal);
    }
}
