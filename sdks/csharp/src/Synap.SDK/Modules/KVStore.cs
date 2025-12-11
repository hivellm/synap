using System.Text.Json;

namespace Synap.SDK.Modules;

/// <summary>
/// Key-Value Store operations.
/// </summary>
public sealed class KVStore
{
    private readonly SynapClient _client;

    internal KVStore(SynapClient client)
    {
        _client = client;
    }

    /// <summary>
    /// Sets a key-value pair.
    /// </summary>
    /// <param name="key">The key to set.</param>
    /// <param name="value">The value to store.</param>
    /// <param name="ttl">Optional time-to-live in seconds.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task SetAsync(
        string key,
        object? value,
        int? ttl = null,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["value"] = value };

        if (ttl.HasValue)
        {
            data["ttl"] = ttl.Value;
        }

        using var response = await _client.ExecuteAsync("kv.set", key, data, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Gets a value by key.
    /// </summary>
    /// <param name="key">The key to get.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The value as a JSON element, or null if not found.</returns>
    public async Task<JsonElement?> GetAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("kv.get", key, null, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("value", out var value))
        {
            // Clone the element to avoid using disposed JsonDocument
            return value.ValueKind != JsonValueKind.Null ? value.Clone() : null;
        }

        return null;
    }

    /// <summary>
    /// Gets a strongly-typed value by key.
    /// </summary>
    /// <typeparam name="T">The type to deserialize to.</typeparam>
    /// <param name="key">The key to get.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The deserialized value, or default if not found.</returns>
    public async Task<T?> GetAsync<T>(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("kv.get", key, null, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("value", out var value) && value.ValueKind != JsonValueKind.Null)
        {
            return value.Deserialize<T>();
        }

        return default;
    }

    /// <summary>
    /// Deletes a key.
    /// </summary>
    /// <param name="key">The key to delete.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    public async Task DeleteAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("kv.delete", key, null, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Checks if a key exists.
    /// </summary>
    /// <param name="key">The key to check.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>True if the key exists, false otherwise.</returns>
    public async Task<bool> ExistsAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("kv.exists", key, null, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("exists", out var exists))
        {
            return exists.GetBoolean();
        }

        return false;
    }

    /// <summary>
    /// Increments a numeric value.
    /// </summary>
    /// <param name="key">The key to increment.</param>
    /// <param name="delta">The amount to increment by.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The new value after incrementing.</returns>
    public async Task<long> IncrAsync(string key, int delta = 1, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["delta"] = delta };
        using var response = await _client.ExecuteAsync("kv.incr", key, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("value", out var value))
        {
            return value.GetInt64();
        }

        return 0;
    }

    /// <summary>
    /// Decrements a numeric value.
    /// </summary>
    /// <param name="key">The key to decrement.</param>
    /// <param name="delta">The amount to decrement by.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The new value after decrementing.</returns>
    public async Task<long> DecrAsync(string key, int delta = 1, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["delta"] = delta };
        using var response = await _client.ExecuteAsync("kv.decr", key, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("value", out var value))
        {
            return value.GetInt64();
        }

        return 0;
    }

    /// <summary>
    /// Gets KV store statistics.
    /// </summary>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Statistics as a dictionary.</returns>
    public async Task<Dictionary<string, JsonElement>> StatsAsync(CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("kv.stats", "*", null, cancellationToken).ConfigureAwait(false);

        var result = new Dictionary<string, JsonElement>();
        foreach (var property in response.RootElement.EnumerateObject())
        {
            result[property.Name] = property.Value;
        }

        return result;
    }

    /// <summary>
    /// Scans keys by prefix.
    /// </summary>
    /// <param name="prefix">The prefix to search for.</param>
    /// <param name="limit">Maximum number of keys to return.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>List of matching keys.</returns>
    public async Task<List<string>> ScanAsync(
        string prefix,
        int limit = 100,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["limit"] = limit };
        using var response = await _client.ExecuteAsync("kv.scan", prefix, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("keys", out var keys) && keys.ValueKind == JsonValueKind.Array)
        {
            var result = new List<string>();
            foreach (var key in keys.EnumerateArray())
            {
                var keyStr = key.GetString();
                if (keyStr is not null)
                {
                    result.Add(keyStr);
                }
            }
            return result;
        }

        return new List<string>();
    }
}

