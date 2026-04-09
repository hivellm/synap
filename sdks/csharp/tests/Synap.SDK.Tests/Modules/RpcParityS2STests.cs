using System.Text.Json;
using Synap.SDK;
using Synap.SDK.Exceptions;
using Xunit;

namespace Synap.SDK.Tests.Modules;

/// <summary>
/// RPC-parity S2S tests — queues, streams, pub/sub, transactions, scripts.
///
/// Tests run across all three transports: HTTP, SynapRPC (synap://), RESP3 (resp3://).
/// All tests are skipped by default; set SYNAP_S2S=true to run them.
///
/// Optional env vars:
///   SYNAP_HTTP_URL   (default: http://localhost:15500)
///   SYNAP_RPC_URL    (default: synap://localhost:15501)
///   SYNAP_RESP3_URL  (default: resp3://localhost:6379)
/// </summary>
public sealed class RpcParityS2STests : IDisposable
{
    private static bool S2SEnabled =>
        Environment.GetEnvironmentVariable("SYNAP_S2S") == "true";

    private static string HttpUrl =>
        Environment.GetEnvironmentVariable("SYNAP_HTTP_URL") ?? "http://localhost:15500";

    private static string RpcUrl =>
        Environment.GetEnvironmentVariable("SYNAP_RPC_URL") ?? "synap://localhost:15501";

    private static string Resp3Url =>
        Environment.GetEnvironmentVariable("SYNAP_RESP3_URL") ?? "resp3://localhost:6379";

    private static SynapClient HttpClient() => new(SynapConfig.Create(HttpUrl));
    private static SynapClient RpcClient()  => new(SynapConfig.Create(RpcUrl));
    private static SynapClient Resp3Client() => new(SynapConfig.Create(Resp3Url));

    private static string Uid() => Guid.NewGuid().ToString("N")[..8];

    public void Dispose() { }

    // ── Queue tests ──────────────────────────────────────────────────────────

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Queue_RoundTrip_Http()
    {
        if (!S2SEnabled) return;
        using var client = HttpClient();
        await QueueRoundTrip(client);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Queue_RoundTrip_Rpc()
    {
        if (!S2SEnabled) return;
        using var client = RpcClient();
        await QueueRoundTrip(client);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Queue_RoundTrip_Resp3()
    {
        if (!S2SEnabled) return;
        using var client = Resp3Client();
        await QueueRoundTrip(client);
    }

    private static async Task QueueRoundTrip(SynapClient client)
    {
        var name = $"test-q-{Uid()}";
        await client.Queue.CreateQueueAsync(name, 100, 60);

        var msgId = await client.Queue.PublishAsync(name, new { data = "hello" }, 5);
        Assert.NotEmpty(msgId);

        var msg = await client.Queue.ConsumeAsync(name, "worker-1");
        Assert.NotNull(msg);
        Assert.Equal(5, msg.Priority);

        await client.Queue.AckAsync(name, msg.Id);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Queue_EmptyReturnsNull_Http()
    {
        if (!S2SEnabled) return;
        using var client = HttpClient();
        var name = $"test-q-empty-{Uid()}";
        await client.Queue.CreateQueueAsync(name);
        var msg = await client.Queue.ConsumeAsync(name, "w1");
        Assert.Null(msg);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Queue_List_Http()
    {
        if (!S2SEnabled) return;
        using var client = HttpClient();
        var name = $"test-q-list-{Uid()}";
        await client.Queue.CreateQueueAsync(name);
        var queues = await client.Queue.ListAsync();
        Assert.Contains(name, queues);
    }

    // ── Stream tests ─────────────────────────────────────────────────────────

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Stream_RoundTrip_Http()
    {
        if (!S2SEnabled) return;
        using var client = HttpClient();
        await StreamRoundTrip(client);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Stream_RoundTrip_Rpc()
    {
        if (!S2SEnabled) return;
        using var client = RpcClient();
        await StreamRoundTrip(client);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Stream_RoundTrip_Resp3()
    {
        if (!S2SEnabled) return;
        using var client = Resp3Client();
        await StreamRoundTrip(client);
    }

    private static async Task StreamRoundTrip(SynapClient client)
    {
        var room = $"test-room-{Uid()}";
        await client.Stream.CreateRoomAsync(room);

        var off0 = await client.Stream.PublishAsync(room, "user.created", new { userId = "u1" });
        var off1 = await client.Stream.PublishAsync(room, "user.updated", new { userId = "u1", name = "Alice" });
        Assert.True(off1 > off0);

        var events = await client.Stream.ReadAsync(room, 0);
        Assert.True(events.Count >= 2);
        Assert.Equal("user.created", events[0].Event);
        Assert.Equal("user.updated", events[1].Event);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Stream_ListRooms_Http()
    {
        if (!S2SEnabled) return;
        using var client = HttpClient();
        var room = $"test-room-list-{Uid()}";
        await client.Stream.CreateRoomAsync(room);
        var rooms = await client.Stream.ListRoomsAsync();
        Assert.Contains(room, rooms);
    }

    // ── Pub/Sub tests ────────────────────────────────────────────────────────

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task PubSub_Publish_Http()
    {
        if (!S2SEnabled) return;
        using var client = HttpClient();
        var count = await client.PubSub.PublishAsync($"test.pub.{Uid()}", new { msg = "hello" });
        Assert.True(count >= 0);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task PubSub_Publish_Rpc()
    {
        if (!S2SEnabled) return;
        using var client = RpcClient();
        var count = await client.PubSub.PublishAsync($"test.pub.{Uid()}", new { msg = "hello" });
        Assert.True(count >= 0);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task PubSub_Publish_Resp3()
    {
        if (!S2SEnabled) return;
        using var client = Resp3Client();
        var count = await client.PubSub.PublishAsync($"test.pub.{Uid()}", new { msg = "hello" });
        Assert.True(count >= 0);
    }

    // ── Transaction tests ────────────────────────────────────────────────────

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Transaction_MultiExec_Http()
    {
        if (!S2SEnabled) return;
        using var client = HttpClient();
        await TransactionRoundTrip(client);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Transaction_MultiExec_Rpc()
    {
        if (!S2SEnabled) return;
        using var client = RpcClient();
        await TransactionRoundTrip(client);
    }

    private static async Task TransactionRoundTrip(SynapClient client)
    {
        var clientId = $"txn-{Uid()}";
        var key      = $"tx:test:{Uid()}";

        using var _ = await client.SendCommandAsync("transaction.multi",
            new Dictionary<string, object?> { ["client_id"] = clientId }).ConfigureAwait(false);
        using var _2 = await client.SendCommandAsync("kv.set",
            new Dictionary<string, object?> { ["key"] = key, ["value"] = "txn-value", ["client_id"] = clientId }).ConfigureAwait(false);
        using var result = await client.SendCommandAsync("transaction.exec",
            new Dictionary<string, object?> { ["client_id"] = clientId }).ConfigureAwait(false);

        Assert.True(result.RootElement.TryGetProperty("success", out var s) && s.GetBoolean());
        var value = await client.KV.GetAsync<string>(key);
        Assert.Equal("txn-value", value);
    }

    // ── Script tests ─────────────────────────────────────────────────────────

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Script_Eval_Http()
    {
        if (!S2SEnabled) return;
        using var client = HttpClient();
        await ScriptEval(client);
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Script_Eval_Rpc()
    {
        if (!S2SEnabled) return;
        using var client = RpcClient();
        await ScriptEval(client);
    }

    private static async Task ScriptEval(SynapClient client)
    {
        using var response = await client.SendCommandAsync("script.eval",
            new Dictionary<string, object?>
            {
                ["script"] = "return 42",
                ["keys"]   = Array.Empty<object?>(),
                ["args"]   = Array.Empty<object?>(),
            });
        Assert.NotNull(response);
    }

    // ── UnsupportedCommandException regression ──────────────────────────────

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Rpc_Raises_UnsupportedCommandException_ForBitmap()
    {
        if (!S2SEnabled) return;
        using var client = RpcClient();
        await Assert.ThrowsAsync<UnsupportedCommandException>(() =>
            client.SendCommandAsync("bitmap.setbit",
                new Dictionary<string, object?> { ["key"] = "bm", ["offset"] = 7L, ["value"] = 1L }));
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Resp3_Raises_UnsupportedCommandException_ForBitmap()
    {
        if (!S2SEnabled) return;
        using var client = Resp3Client();
        await Assert.ThrowsAsync<UnsupportedCommandException>(() =>
            client.SendCommandAsync("bitmap.setbit",
                new Dictionary<string, object?> { ["key"] = "bm", ["offset"] = 7L, ["value"] = 1L }));
    }

    [Fact(Skip = "S2S — set SYNAP_S2S=true to run")]
    public async Task Http_DoesNotRaise_UnsupportedCommandException_ForBitmap()
    {
        if (!S2SEnabled) return;
        using var client = HttpClient();
        var ex = await Record.ExceptionAsync(() =>
            client.SendCommandAsync("bitmap.setbit",
                new Dictionary<string, object?> { ["key"] = $"bm:{Uid()}", ["offset"] = 7L, ["value"] = 1L }));
        Assert.IsNotType<UnsupportedCommandException>(ex);
    }
}
