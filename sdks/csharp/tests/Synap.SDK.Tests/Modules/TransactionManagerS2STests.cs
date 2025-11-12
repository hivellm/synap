using System.Diagnostics;
using Synap.SDK;
using Synap.SDK.Modules;
using Xunit;

namespace Synap.SDK.Tests.Modules;

/// <summary>
/// S2S integration tests for TransactionManager.
/// These tests require a running Synap server.
/// </summary>
public class TransactionManagerS2STests
{
    private readonly SynapClient _client;

    public TransactionManagerS2STests()
    {
        var url = Environment.GetEnvironmentVariable("SYNAP_URL") ?? "http://localhost:15500";
        _client = new SynapClient(new SynapConfig(url));
    }

    [Fact]
    public async Task MultiExec_Works()
    {
        var clientId = $"test:{Guid.NewGuid()}";

        // Start transaction
        var result = await _client.Transaction.MultiAsync(clientId);
        Assert.True(result.Success);

        // Queue commands using ExecuteAsync with client_id (automatic queuing)
        var setData1 = new Dictionary<string, object?> { ["key"] = "tx:key1", ["value"] = "value1", ["client_id"] = clientId };
        var setData2 = new Dictionary<string, object?> { ["key"] = "tx:key2", ["value"] = "value2", ["client_id"] = clientId };
        await _client.ExecuteAsync("kv.set", string.Empty, setData1);
        await _client.ExecuteAsync("kv.set", string.Empty, setData2);

        // Execute transaction
        var execResult = await _client.Transaction.ExecAsync(clientId);
        Assert.True(execResult is TransactionExecSuccess);
        var success = (TransactionExecSuccess)execResult;
        Assert.Equal(2, success.Results.Count);

        // Verify values were set
        var value1 = await _client.KV.GetAsync<string>("tx:key1");
        var value2 = await _client.KV.GetAsync<string>("tx:key2");
        Assert.Equal("value1", value1);
        Assert.Equal("value2", value2);
    }

    [Fact]
    public async Task Discard_Works()
    {
        var clientId = $"test:{Guid.NewGuid()}";

        // Start transaction
        await _client.Transaction.MultiAsync(clientId);

        // Queue a command (will be discarded)
        var setData = new Dictionary<string, object?> { ["key"] = "tx:discard:key", ["value"] = "value", ["client_id"] = clientId };
        await _client.ExecuteAsync("kv.set", string.Empty, setData);

        // Discard transaction
        var result = await _client.Transaction.DiscardAsync(clientId);
        Assert.True(result.Success);

        // Verify value was NOT set
        var value = await _client.KV.GetAsync<string>("tx:discard:key");
        Assert.Null(value);
    }

    [Fact]
    public async Task WatchUnwatch_Works()
    {
        var clientId = $"test:{Guid.NewGuid()}";

        // Start transaction
        await _client.Transaction.MultiAsync(clientId);

        // Watch keys
        var result = await _client.Transaction.WatchAsync(
            new List<string> { "watch:key1", "watch:key2" },
            clientId
        );
        Assert.True(result.Success);

        // Unwatch
        result = await _client.Transaction.UnwatchAsync(clientId);
        Assert.True(result.Success);
    }

    [Fact]
    public async Task WatchAbortOnConflict_Works()
    {
        var clientId = $"test:{Guid.NewGuid()}";

        // Set initial value
        await _client.KV.SetAsync("watch:conflict:key", "initial");

        // Start transaction and watch
        await _client.Transaction.MultiAsync(clientId);
        await _client.Transaction.WatchAsync(
            new List<string> { "watch:conflict:key" },
            clientId
        );

        // Modify watched key from another client (simulate conflict)
        await _client.KV.SetAsync("watch:conflict:key", "modified");

        // Try to execute transaction (should abort)
        var execResult = await _client.Transaction.ExecAsync(clientId);
        Assert.True(execResult is TransactionExecAborted);
        var aborted = (TransactionExecAborted)execResult;
        Assert.True(aborted.Aborted);
    }

    [Fact]
    public async Task EmptyTransaction_Works()
    {
        var clientId = $"test:{Guid.NewGuid()}";

        // Start transaction
        await _client.Transaction.MultiAsync(clientId);

        // Execute without queuing commands
        var execResult = await _client.Transaction.ExecAsync(clientId);
        Assert.True(execResult is TransactionExecSuccess);
        var success = (TransactionExecSuccess)execResult;
        Assert.Empty(success.Results);
    }
}

