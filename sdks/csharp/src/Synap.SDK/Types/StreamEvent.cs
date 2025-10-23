using System.Text.Json;

namespace Synap.SDK.Types;

/// <summary>
/// Represents an event from a stream.
/// </summary>
public sealed record StreamEvent
{
    /// <summary>
    /// Gets the event offset in the stream.
    /// </summary>
    public required long Offset { get; init; }

    /// <summary>
    /// Gets the event type/name.
    /// </summary>
    public required string Event { get; init; }

    /// <summary>
    /// Gets the event data.
    /// </summary>
    public required JsonElement Data { get; init; }

    /// <summary>
    /// Gets the event timestamp (Unix timestamp in seconds).
    /// </summary>
    public long Timestamp { get; init; }

    /// <summary>
    /// Gets the room name.
    /// </summary>
    public string? Room { get; init; }
}

