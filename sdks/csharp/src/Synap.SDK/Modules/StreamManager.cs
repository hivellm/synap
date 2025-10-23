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

    /// <summary>
    /// Creates a new stream room.
    /// </summary>
    /// <param name="room">The room name.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task CreateRoomAsync(string room, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("stream.create_room", room, null, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Deletes a stream room.
    /// </summary>
    /// <param name="room">The room name.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task DeleteRoomAsync(string room, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("stream.delete_room", room, null, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Publishes an event to a stream room.
    /// </summary>
    /// <param name="room">The room name.</param>
    /// <param name="event">The event type/name.</param>
    /// <param name="data">The event data.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The event offset in the stream.</returns>
    public async Task<long> PublishAsync(
        string room,
        string @event,
        object? data,
        CancellationToken cancellationToken = default)
    {
        var requestData = new Dictionary<string, object?>
        {
            ["event"] = @event,
            ["data"] = data
        };

        using var response = await _client.ExecuteAsync("stream.publish", room, requestData, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("offset", out var offset))
        {
            return offset.GetInt64();
        }

        return 0;
    }

    /// <summary>
    /// Reads events from a stream.
    /// </summary>
    /// <param name="room">The room name.</param>
    /// <param name="offset">Starting offset (0 for beginning).</param>
    /// <param name="limit">Maximum number of events to read.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>List of stream events.</returns>
    public async Task<List<StreamEvent>> ReadAsync(
        string room,
        long offset = 0,
        int limit = 100,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["offset"] = offset,
            ["limit"] = limit
        };

        using var response = await _client.ExecuteAsync("stream.read", room, data, cancellationToken).ConfigureAwait(false);

        if (!response.RootElement.TryGetProperty("events", out var events) ||
            events.ValueKind != JsonValueKind.Array)
        {
            return new List<StreamEvent>();
        }

        var result = new List<StreamEvent>();
        foreach (var eventElement in events.EnumerateArray())
        {
            var streamEvent = new StreamEvent
            {
                Offset = eventElement.TryGetProperty("offset", out var off) ? off.GetInt64() : 0,
                Event = eventElement.TryGetProperty("event", out var evt) ? evt.GetString() ?? string.Empty : string.Empty,
                Data = eventElement.TryGetProperty("data", out var dat) ? dat : default,
                Timestamp = eventElement.TryGetProperty("timestamp", out var ts) ? ts.GetInt64() : 0,
                Room = eventElement.TryGetProperty("room", out var rm) ? rm.GetString() : null
            };
            result.Add(streamEvent);
        }

        return result;
    }

    /// <summary>
    /// Gets stream statistics.
    /// </summary>
    /// <param name="room">The room name.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Statistics as a dictionary.</returns>
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

    /// <summary>
    /// Lists all stream rooms.
    /// </summary>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>List of room names.</returns>
    public async Task<List<string>> ListRoomsAsync(CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("stream.list_rooms", "*", null, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("rooms", out var rooms) && rooms.ValueKind == JsonValueKind.Array)
        {
            var result = new List<string>();
            foreach (var room in rooms.EnumerateArray())
            {
                var roomStr = room.GetString();
                if (roomStr is not null)
                {
                    result.Add(roomStr);
                }
            }
            return result;
        }

        return new List<string>();
    }
}

