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
    /// Subscribes to topics for a subscriber.
    /// </summary>
    /// <param name="subscriberId">The subscriber ID.</param>
    /// <param name="topics">List of topic patterns (supports wildcards like 'user.*').</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task SubscribeTopicsAsync(
        string subscriberId,
        IEnumerable<string> topics,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["topics"] = topics };
        using var response = await _client.ExecuteAsync("pubsub.subscribe", subscriberId, data, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Unsubscribes from topics for a subscriber.
    /// </summary>
    /// <param name="subscriberId">The subscriber ID.</param>
    /// <param name="topics">List of topic patterns to unsubscribe from.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task UnsubscribeTopicsAsync(
        string subscriberId,
        IEnumerable<string> topics,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["topics"] = topics };
        using var response = await _client.ExecuteAsync("pubsub.unsubscribe", subscriberId, data, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Publishes a message to a topic.
    /// </summary>
    /// <param name="topic">The topic name.</param>
    /// <param name="message">The message payload.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Number of subscribers that received the message.</returns>
    public async Task<int> PublishAsync(
        string topic,
        object? message,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["message"] = message };
        using var response = await _client.ExecuteAsync("pubsub.publish", topic, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("delivered", out var delivered))
        {
            return delivered.GetInt32();
        }

        return 0;
    }

    /// <summary>
    /// Gets Pub/Sub statistics.
    /// </summary>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Statistics as a dictionary.</returns>
    public async Task<Dictionary<string, JsonElement>> StatsAsync(CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("pubsub.stats", "*", null, cancellationToken).ConfigureAwait(false);

        var result = new Dictionary<string, JsonElement>();
        foreach (var property in response.RootElement.EnumerateObject())
        {
            result[property.Name] = property.Value;
        }

        return result;
    }
}

