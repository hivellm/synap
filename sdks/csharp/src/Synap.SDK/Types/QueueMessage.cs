using System.Text.Json;

namespace Synap.SDK.Types;

/// <summary>
/// Represents a message from a queue.
/// </summary>
public sealed record QueueMessage
{
    /// <summary>
    /// Gets the message ID.
    /// </summary>
    public required string Id { get; init; }

    /// <summary>
    /// Gets the message payload.
    /// </summary>
    public required JsonElement? Payload { get; init; }

    /// <summary>
    /// Gets the message priority (0-9, higher is more important).
    /// </summary>
    public int Priority { get; init; }

    /// <summary>
    /// Gets the number of times this message has been retried.
    /// </summary>
    public int Retries { get; init; }

    /// <summary>
    /// Gets the maximum number of retries allowed.
    /// </summary>
    public int MaxRetries { get; init; } = 3;

    /// <summary>
    /// Gets the message timestamp (Unix timestamp in seconds).
    /// </summary>
    public long Timestamp { get; init; }
}

