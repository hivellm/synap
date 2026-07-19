using System.Text.Json;
using System.Text.Json.Serialization;

namespace Synap.SDK.Types;

/// <summary>
/// One KV watch envelope (<c>docs/kv-watch.md</c> in the server repository).
/// </summary>
/// <remarks>
/// <see cref="Value"/> is the <b>post-mutation</b> value and is <c>null</c> for
/// terminal events (<c>del</c>, <c>expired</c>, <c>evicted</c>), TTL-only
/// events (<c>expire</c>, <c>persist</c>), and envelopes degraded to
/// notify-only (<see cref="Truncated"/> is <c>true</c>).
/// </remarks>
public sealed record WatchEvent
{
    /// <summary>Gets the key that changed.</summary>
    [JsonPropertyName("key")]
    public required string Key { get; init; }

    /// <summary>
    /// Gets what happened: <c>set</c>, <c>del</c>, <c>expired</c>,
    /// <c>evicted</c>, <c>expire</c>, <c>persist</c>, <c>append</c>, ...
    /// </summary>
    [JsonPropertyName("event")]
    public required string Event { get; init; }

    /// <summary>
    /// Gets the per-key counter for gap detection. Resets when the key is
    /// deleted, expires or is evicted — version 1 marks a new incarnation.
    /// </summary>
    [JsonPropertyName("version")]
    public required long Version { get; init; }

    /// <summary>Gets the post-mutation value, when inlined.</summary>
    [JsonPropertyName("value")]
    public string? Value { get; init; }

    /// <summary>
    /// Gets a value indicating whether the value was withheld (over the inline
    /// cap, or not UTF-8).
    /// </summary>
    [JsonPropertyName("truncated")]
    public bool Truncated { get; init; }

    /// <summary>
    /// Decodes a watch envelope from its JSON form, or returns <c>null</c>
    /// when the payload is not a watch envelope.
    /// </summary>
    /// <param name="json">The envelope JSON text.</param>
    /// <returns>The decoded event, or <c>null</c>.</returns>
    public static WatchEvent? FromJson(string json)
    {
        try
        {
            return JsonSerializer.Deserialize<WatchEvent>(json);
        }
        catch (JsonException)
        {
            return null;
        }
    }
}

/// <summary>Per-subscription delivery mode for KV watch.</summary>
public enum WatchMode
{
    /// <summary>Envelopes carry the post-mutation value (up to the server's inline cap).</summary>
    Value,

    /// <summary>
    /// Envelopes carry key/event/version only; the server strips the value per
    /// subscription, so a watcher that only wants change signals pays no value
    /// bandwidth.
    /// </summary>
    Notify,
}
