using System.Text.Json;
using Synap.SDK.Types;

namespace Synap.SDK.Modules;

/// <summary>
/// Event Stream operations.
/// </summary>
public sealed class StreamManager
{
    private readonly SynapClient _client;

    internal StreamManager(SynapClient client)
    {
        _client = client;
    }

    /// <summary>Creates a new stream room.</summary>
    public async Task CreateRoomAsync(string room, CancellationToken cancellationToken = default)
    {
        using var _ = await _client.SendCommandAsync(
            "stream.create",
            new Dictionary<string, object?> { ["room"] = room },
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Deletes a stream room.</summary>
    public async Task DeleteRoomAsync(string room, CancellationToken cancellationToken = default)
    {
        using var _ = await _client.SendCommandAsync(
            "stream.delete",
            new Dictionary<string, object?> { ["room"] = room },
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Publishes an event to a stream room.</summary>
    /// <returns>The event offset in the stream.</returns>
    public async Task<long> PublishAsync(
        string room,
        string @event,
        object? data,
        CancellationToken cancellationToken = default)
    {
        using var response = await _client.SendCommandAsync(
            "stream.publish",
            new Dictionary<string, object?>
            {
                ["room"]  = room,
                ["event"] = @event,
                ["data"]  = data,
            },
            cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("offset", out var offset) ? offset.GetInt64() : 0L;
    }

    /// <summary>Reads events from a stream.</summary>
    public async Task<List<StreamEvent>> ReadAsync(
        string room,
        long offset = 0,
        string subscriberId = "sdk-reader",
        CancellationToken cancellationToken = default)
    {
        using var response = await _client.SendCommandAsync(
            "stream.consume",
            new Dictionary<string, object?>
            {
                ["room"]          = room,
                ["subscriber_id"] = subscriberId,
                ["from_offset"]   = offset,
            },
            cancellationToken).ConfigureAwait(false);

        if (!response.RootElement.TryGetProperty("events", out var events) ||
            events.ValueKind != JsonValueKind.Array)
        {
            return new List<StreamEvent>();
        }

        var result = new List<StreamEvent>();
        foreach (var eventElement in events.EnumerateArray())
        {
            result.Add(new StreamEvent
            {
                Offset    = eventElement.TryGetProperty("offset", out var off) ? off.GetInt64() : 0L,
                Event     = eventElement.TryGetProperty("event", out var evt) ? evt.GetString() ?? string.Empty : string.Empty,
                Data      = eventElement.TryGetProperty("data", out var dat) ? dat : default,
                Timestamp = eventElement.TryGetProperty("timestamp", out var ts) ? ts.GetInt64() : 0L,
                Room      = eventElement.TryGetProperty("room", out var rm) ? rm.GetString() : null,
            });
        }

        return result;
    }

    /// <summary>Lists all stream rooms.</summary>
    public async Task<List<string>> ListRoomsAsync(CancellationToken cancellationToken = default)
    {
        using var response = await _client.SendCommandAsync(
            "stream.list",
            new Dictionary<string, object?>(),
            cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("rooms", out var rooms) && rooms.ValueKind == JsonValueKind.Array)
        {
            var result = new List<string>();
            foreach (var room in rooms.EnumerateArray())
            {
                var s = room.GetString();
                if (s is not null)
                {
                    result.Add(s);
                }
            }

            return result;
        }

        return new List<string>();
    }

    /// <summary>Gets stream statistics.</summary>
    public async Task<Dictionary<string, JsonElement>> StatsAsync(
        string room,
        CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("stream.stats", room, null, cancellationToken).ConfigureAwait(false);

        var result = new Dictionary<string, JsonElement>();
        foreach (var property in response.RootElement.EnumerateObject())
        {
            result[property.Name] = property.Value;
        }

        return result;
    }
}
