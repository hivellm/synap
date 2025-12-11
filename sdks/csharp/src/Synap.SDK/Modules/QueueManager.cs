using System.Text.Json;
using Synap.SDK.Types;

namespace Synap.SDK.Modules;

/// <summary>
/// Message Queue operations.
/// </summary>
public sealed class QueueManager
{
    private readonly SynapClient _client;

    internal QueueManager(SynapClient client)
    {
        _client = client;
    }

    /// <summary>
    /// Creates a new queue.
    /// </summary>
    /// <param name="name">The queue name.</param>
    /// <param name="maxSize">Optional maximum queue size.</param>
    /// <param name="messageTtl">Optional message TTL in seconds.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task CreateQueueAsync(
        string name,
        int? maxSize = null,
        int? messageTtl = null,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>();

        if (maxSize.HasValue)
        {
            data["max_size"] = maxSize.Value;
        }

        if (messageTtl.HasValue)
        {
            data["message_ttl"] = messageTtl.Value;
        }

        using var response = await _client.ExecuteAsync("queue.create", name, data, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Deletes a queue.
    /// </summary>
    /// <param name="name">The queue name.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task DeleteQueueAsync(string name, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("queue.delete", name, null, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Publishes a message to a queue.
    /// </summary>
    /// <param name="queue">The queue name.</param>
    /// <param name="message">The message payload.</param>
    /// <param name="priority">Message priority (0-9, higher is more important).</param>
    /// <param name="maxRetries">Maximum retry attempts.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The message ID.</returns>
    public async Task<string> PublishAsync(
        string queue,
        object? message,
        int? priority = null,
        int? maxRetries = null,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["message"] = message };

        if (priority.HasValue)
        {
            data["priority"] = priority.Value;
        }

        if (maxRetries.HasValue)
        {
            data["max_retries"] = maxRetries.Value;
        }

        using var response = await _client.ExecuteAsync("queue.publish", queue, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("message_id", out var messageId))
        {
            return messageId.GetString() ?? string.Empty;
        }

        return string.Empty;
    }

    /// <summary>
    /// Consumes a message from a queue.
    /// </summary>
    /// <param name="queue">The queue name.</param>
    /// <param name="consumerId">The consumer ID.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The queue message, or null if no message is available.</returns>
    public async Task<QueueMessage?> ConsumeAsync(
        string queue,
        string consumerId,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["consumer_id"] = consumerId };
        using var response = await _client.ExecuteAsync("queue.consume", queue, data, cancellationToken).ConfigureAwait(false);

        if (!response.RootElement.TryGetProperty("message", out var messageElement))
        {
            return null;
        }

        return new QueueMessage
        {
            Id = messageElement.TryGetProperty("id", out var id) ? id.GetString() ?? string.Empty : string.Empty,
            Payload = messageElement.TryGetProperty("payload", out var payload) ? payload : (JsonElement?)null,
            Priority = messageElement.TryGetProperty("priority", out var priority) ? priority.GetInt32() : 0,
            Retries = messageElement.TryGetProperty("retries", out var retries) ? retries.GetInt32() : 0,
            MaxRetries = messageElement.TryGetProperty("max_retries", out var maxRetries) ? maxRetries.GetInt32() : 3,
            Timestamp = messageElement.TryGetProperty("timestamp", out var timestamp) ? timestamp.GetInt64() : 0
        };
    }

    /// <summary>
    /// Acknowledges successful message processing.
    /// </summary>
    /// <param name="queue">The queue name.</param>
    /// <param name="messageId">The message ID to acknowledge.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task AckAsync(string queue, string messageId, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["message_id"] = messageId };
        using var response = await _client.ExecuteAsync("queue.ack", queue, data, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Negative acknowledges a message (requeues it for retry).
    /// </summary>
    /// <param name="queue">The queue name.</param>
    /// <param name="messageId">The message ID to requeue.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task NackAsync(string queue, string messageId, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["message_id"] = messageId };
        using var response = await _client.ExecuteAsync("queue.nack", queue, data, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Gets queue statistics.
    /// </summary>
    /// <param name="queue">The queue name.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Statistics as a dictionary.</returns>
    public async Task<Dictionary<string, JsonElement>> StatsAsync(
        string queue,
        CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("queue.stats", queue, null, cancellationToken).ConfigureAwait(false);

        var result = new Dictionary<string, JsonElement>();
        foreach (var property in response.RootElement.EnumerateObject())
        {
            result[property.Name] = property.Value;
        }

        return result;
    }

    /// <summary>
    /// Lists all queues.
    /// </summary>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>List of queue names.</returns>
    public async Task<List<string>> ListAsync(CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("queue.list", "*", null, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("queues", out var queues) && queues.ValueKind == JsonValueKind.Array)
        {
            var result = new List<string>();
            foreach (var queue in queues.EnumerateArray())
            {
                var queueStr = queue.GetString();
                if (queueStr is not null)
                {
                    result.Add(queueStr);
                }
            }
            return result;
        }

        return new List<string>();
    }
}

