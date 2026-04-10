using System.Text.Json;

namespace Synap.SDK.Modules;

/// <summary>
/// Pub/Sub operations.
/// </summary>
public sealed class PubSubManager
{
    private readonly SynapClient _client;

    internal PubSubManager(SynapClient client)
    {
        _client = client;
    }

    /// <summary>
    /// Publishes a message to a topic.
    /// </summary>
    /// <returns>Number of subscribers that received the message.</returns>
    public async Task<long> PublishAsync(
        string topic,
        object? message,
        CancellationToken cancellationToken = default)
    {
        using var response = await _client.SendCommandAsync(
            "pubsub.publish",
            new Dictionary<string, object?> { ["topic"] = topic, ["payload"] = message },
            cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("subscribers_matched", out var sm)
            ? sm.GetInt64()
            : 0L;
    }

    /// <summary>
    /// Registers a subscription on the server (HTTP transport only).
    /// For real-time delivery on SynapRPC, use <see cref="ObserveAsync"/>.
    /// </summary>
    public async Task SubscribeTopicsAsync(
        string subscriberId,
        IEnumerable<string> topics,
        CancellationToken cancellationToken = default)
    {
        using var _ = await _client.SendCommandAsync(
            "pubsub.subscribe",
            new Dictionary<string, object?>
            {
                ["subscriber_id"] = subscriberId,
                ["topics"]        = topics.Cast<object?>().ToArray(),
            },
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Unsubscribes from topics.</summary>
    public async Task UnsubscribeTopicsAsync(
        string subscriberId,
        IEnumerable<string> topics,
        CancellationToken cancellationToken = default)
    {
        using var _ = await _client.SendCommandAsync(
            "pubsub.unsubscribe",
            new Dictionary<string, object?>
            {
                ["subscriber_id"] = subscriberId,
                ["topics"]        = topics.Cast<object?>().ToArray(),
            },
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Gets Pub/Sub statistics.</summary>
    public async Task<Dictionary<string, JsonElement>> StatsAsync(CancellationToken cancellationToken = default)
    {
        using var response = await _client.SendCommandAsync("pubsub.stats", null, cancellationToken).ConfigureAwait(false);

        var result = new Dictionary<string, JsonElement>();
        foreach (var property in response.RootElement.EnumerateObject())
        {
            result[property.Name] = property.Value;
        }

        return result;
    }

    /// <summary>
    /// Subscribe and yield push messages as an async stream (SynapRPC only).
    ///
    /// On a SynapRPC transport this opens a dedicated server-push TCP connection
    /// and yields messages in real time.  On other transports this registers the
    /// subscription via HTTP and returns an empty stream (no real-time delivery).
    /// </summary>
    /// <param name="topics">Topic patterns to subscribe to.</param>
    /// <param name="cancellationToken">Token used to stop the stream.</param>
    /// <returns>Async stream of message dictionaries (keys: topic, payload, id, timestamp).</returns>
    public async IAsyncEnumerable<Dictionary<string, object?>> ObserveAsync(
        IEnumerable<string> topics,
        [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        var rpc = _client.GetSynapRpcTransport();
        if (rpc is not null)
        {
            await foreach (var msg in rpc.SubscribePushAsync(topics, cancellationToken).ConfigureAwait(false))
            {
                yield return msg;
            }
        }
        else
        {
            // HTTP fallback — register subscription, no real-time delivery
            var sid = $"cs-sub-{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()}";
            await SubscribeTopicsAsync(sid, topics, cancellationToken).ConfigureAwait(false);
            // Yield nothing — callers must poll the HTTP API
        }
    }
}
