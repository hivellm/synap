using System.Text.Json;

namespace Synap.SDK.Modules;

/// <summary>
/// List data structure operations (Redis-compatible).
/// </summary>
public sealed class ListManager
{
    private readonly SynapClient _client;

    public ListManager(SynapClient client)
    {
        _client = client;
    }

    /// <summary>
    /// Push elements to left (head) of list.
    /// </summary>
    public async Task<int> LPushAsync(
        string key,
        IReadOnlyList<string> values,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["values"] = values };
        using var response = await _client.ExecuteAsync("list.lpush", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("length", out var length) ? length.GetInt32() : 0;
    }

    /// <summary>
    /// Push elements to right (tail) of list.
    /// </summary>
    public async Task<int> RPushAsync(
        string key,
        IReadOnlyList<string> values,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["values"] = values };
        using var response = await _client.ExecuteAsync("list.rpush", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("length", out var length) ? length.GetInt32() : 0;
    }

    /// <summary>
    /// Pop elements from left (head) of list.
    /// </summary>
    public async Task<List<string>> LPopAsync(
        string key,
        int count = 1,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["count"] = count };
        using var response = await _client.ExecuteAsync("list.lpop", key, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("values", out var values))
        {
            return JsonSerializer.Deserialize<List<string>>(values.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Pop elements from right (tail) of list.
    /// </summary>
    public async Task<List<string>> RPopAsync(
        string key,
        int count = 1,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["count"] = count };
        using var response = await _client.ExecuteAsync("list.rpop", key, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("values", out var values))
        {
            return JsonSerializer.Deserialize<List<string>>(values.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Get range of elements from list.
    /// </summary>
    public async Task<List<string>> RangeAsync(
        string key,
        int start = 0,
        int stop = -1,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["start"] = start,
            ["stop"] = stop
        };

        using var response = await _client.ExecuteAsync("list.range", key, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("values", out var values))
        {
            return JsonSerializer.Deserialize<List<string>>(values.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Get list length.
    /// </summary>
    public async Task<int> LenAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("list.len", key, null, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("length", out var length) ? length.GetInt32() : 0;
    }

    /// <summary>
    /// Get element at index.
    /// </summary>
    public async Task<string?> IndexAsync(string key, int index, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["index"] = index };
        using var response = await _client.ExecuteAsync("list.index", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("value", out var value) ? value.GetString() : null;
    }

    /// <summary>
    /// Set element at index.
    /// </summary>
    public async Task<bool> SetAsync(
        string key,
        int index,
        string value,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["index"] = index,
            ["value"] = value
        };

        using var response = await _client.ExecuteAsync("list.set", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("success", out var success) && success.GetBoolean();
    }

    /// <summary>
    /// Trim list to specified range.
    /// </summary>
    public async Task<bool> TrimAsync(
        string key,
        int start,
        int stop,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["start"] = start,
            ["stop"] = stop
        };

        using var response = await _client.ExecuteAsync("list.trim", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("success", out var success) && success.GetBoolean();
    }
}

