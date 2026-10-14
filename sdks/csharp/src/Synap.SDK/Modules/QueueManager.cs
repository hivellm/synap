using System.Text;
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

    /// <summary>Creates a new queue.</summary>
    public async Task CreateQueueAsync(
        string name,
        int? maxSize = null,
        int? messageTtl = null,
        CancellationToken cancellationToken = default)
    {
        using var _ = await _client.SendCommandAsync(
            "queue.create",
            new Dictionary<string, object?>
            {
                ["name"]              = name,
                ["max_depth"]         = (long)(maxSize ?? 0),
                ["ack_deadline_secs"] = (long)(messageTtl ?? 0),
            },
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Deletes a queue.</summary>
    public async Task DeleteQueueAsync(string name, CancellationToken cancellationToken = default)
    {
        using var _ = await _client.SendCommandAsync(
            "queue.delete",
            new Dictionary<string, object?> { ["queue"] = name },
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Publishes a message to a queue.</summary>
    /// <param name="queue">The queue name.</param>
    /// <param name="message">
    /// The message: a <c>byte[]</c> travels as is, a string as its UTF-8 text,
    /// anything else as its UTF-8 JSON.
    /// </param>
    /// <param name="priority">Priority, 0-9 (higher is delivered first).</param>
    /// <param name="maxRetries">Maximum retries before the message is dead-lettered (default 3).</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The message ID.</returns>
    public async Task<string> PublishAsync(
        string queue,
        object? message,
        int? priority = null,
        int? maxRetries = null,
        CancellationToken cancellationToken = default)
    {
        using var response = await _client.SendCommandAsync(
            "queue.publish",
            new Dictionary<string, object?>
            {
                ["queue"]       = queue,
                ["payload"]     = EncodePayload(message),
                ["priority"]    = (long)(priority ?? 0),
                ["max_retries"] = (long)(maxRetries ?? 3),
            },
            cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("message_id", out var mid)
            ? mid.GetString() ?? string.Empty
            : string.Empty;
    }

    /// <summary>Consumes a message from a queue.</summary>
    /// <returns>The queue message, or null if no message is available.</returns>
    public async Task<QueueMessage?> ConsumeAsync(
        string queue,
        string consumerId,
        CancellationToken cancellationToken = default)
    {
        using var response = await _client.SendCommandAsync(
            "queue.consume",
            new Dictionary<string, object?>
            {
                ["queue"]       = queue,
                ["consumer_id"] = consumerId,
            },
            cancellationToken).ConfigureAwait(false);

        // An empty queue answers {"message": null}.
        if (!response.RootElement.TryGetProperty("message", out var messageElement)
            || messageElement.ValueKind != JsonValueKind.Object)
        {
            return null;
        }

        return new QueueMessage
        {
            Id         = messageElement.TryGetProperty("id", out var id) ? id.GetString() ?? string.Empty : string.Empty,
            Payload    = messageElement.TryGetProperty("payload", out var pl) ? DecodePayload(pl) : (JsonElement?)null,
            Priority   = messageElement.TryGetProperty("priority", out var pri) ? pri.GetInt32() : 0,
            Retries    = messageElement.TryGetProperty("retry_count", out var rc) ? rc.GetInt32()
                       : messageElement.TryGetProperty("retries", out var ret) ? ret.GetInt32() : 0,
            MaxRetries = messageElement.TryGetProperty("max_retries", out var mr) ? mr.GetInt32() : 3,
            Timestamp  = messageElement.TryGetProperty("timestamp", out var ts) ? ts.GetInt64() : 0L,
        };
    }

    /// <summary>Acknowledges successful message processing.</summary>
    public async Task AckAsync(string queue, string messageId, CancellationToken cancellationToken = default)
    {
        using var _ = await _client.SendCommandAsync(
            "queue.ack",
            new Dictionary<string, object?> { ["queue"] = queue, ["message_id"] = messageId },
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Negative acknowledges a message (requeues it for retry).</summary>
    public async Task NackAsync(
        string queue,
        string messageId,
        int delaySecs = 0,
        CancellationToken cancellationToken = default)
    {
        using var _ = await _client.SendCommandAsync(
            "queue.nack",
            new Dictionary<string, object?> { ["queue"] = queue, ["message_id"] = messageId, ["delay_secs"] = (long)delaySecs },
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Gets queue statistics.</summary>
    public async Task<Dictionary<string, JsonElement>> StatsAsync(
        string queue,
        CancellationToken cancellationToken = default)
    {
        using var response = await _client.SendCommandAsync(
            "queue.stats",
            new Dictionary<string, object?> { ["queue"] = queue },
            cancellationToken).ConfigureAwait(false);

        var result = new Dictionary<string, JsonElement>();
        foreach (var property in response.RootElement.EnumerateObject())
        {
            result[property.Name] = property.Value;
        }

        return result;
    }

    /// <summary>Lists all queues.</summary>
    public async Task<List<string>> ListAsync(CancellationToken cancellationToken = default)
    {
        using var response = await _client.SendCommandAsync(
            "queue.list",
            new Dictionary<string, object?>(),
            cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("queues", out var queues) && queues.ValueKind == JsonValueKind.Array)
        {
            var result = new List<string>();
            foreach (var q in queues.EnumerateArray())
            {
                var s = q.GetString();
                if (s is not null)
                {
                    result.Add(s);
                }
            }

            return result;
        }

        return new List<string>();
    }

    /// <summary>
    /// The message as the byte list the command wire carries (the TypeScript
    /// and Python SDKs encode it the same way): a <c>byte[]</c> as is, a string
    /// as UTF-8, anything else as its UTF-8 JSON.
    /// </summary>
    private static int[] EncodePayload(object? message)
    {
        var bytes = message switch
        {
            byte[] raw => raw,
            string text => Encoding.UTF8.GetBytes(text),
            _ => JsonSerializer.SerializeToUtf8Bytes(message),
        };
        return Array.ConvertAll(bytes, b => (int)b);
    }

    private static readonly UTF8Encoding StrictUtf8 = new(encoderShouldEmitUTF8Identifier: false, throwOnInvalidBytes: true);

    /// <summary>
    /// The published value back from the wire's byte list: its JSON when the
    /// bytes are JSON, else its text when they are UTF-8, else the byte list
    /// itself. The result is detached from the response document, which is
    /// disposed when <see cref="ConsumeAsync"/> returns.
    /// </summary>
    private static JsonElement DecodePayload(JsonElement payload)
    {
        if (payload.ValueKind != JsonValueKind.Array)
        {
            return payload.Clone();
        }

        byte[] bytes;
        try
        {
            bytes = payload.EnumerateArray().Select(b => b.GetByte()).ToArray();
        }
        catch (Exception ex) when (ex is FormatException or InvalidOperationException)
        {
            // Not a byte list: hand it over as it came.
            return payload.Clone();
        }

        string text;
        try
        {
            text = StrictUtf8.GetString(bytes);
        }
        catch (DecoderFallbackException)
        {
            return payload.Clone();
        }

        try
        {
            using var json = JsonDocument.Parse(bytes);
            return json.RootElement.Clone();
        }
        catch (JsonException)
        {
            return JsonSerializer.SerializeToElement(text);
        }
    }
}
