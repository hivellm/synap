using System.Text.Json;

namespace Synap.SDK.Modules;

/// <summary>
/// Hash data structure operations (Redis-compatible).
/// </summary>
public sealed class HashManager
{
    private readonly SynapClient _client;

    public HashManager(SynapClient client)
    {
        _client = client;
    }

    /// <summary>
    /// Set field in hash.
    /// </summary>
    public async Task<bool> SetAsync(
        string key,
        string field,
        string value,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["field"] = field,
            ["value"] = value
        };

        using var response = await _client.ExecuteAsync("hash.set", key, data, cancellationToken).ConfigureAwait(false);
        return response.RootElement.TryGetProperty("success", out var success) && success.GetBoolean();
    }

    /// <summary>
    /// Get field from hash.
    /// </summary>
    public async Task<string?> GetAsync(string key, string field, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["field"] = field };
        using var response = await _client.ExecuteAsync("hash.get", key, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("value", out var value))
        {
            return value.GetString();
        }

        return null;
    }

    /// <summary>
    /// Get all fields and values from hash.
    /// </summary>
    public async Task<Dictionary<string, string>> GetAllAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("hash.getall", key, null, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("fields", out var fields))
        {
            return JsonSerializer.Deserialize<Dictionary<string, string>>(fields.GetRawText()) ?? new();
        }

        return new Dictionary<string, string>();
    }

    /// <summary>
    /// Delete field from hash.
    /// </summary>
    public async Task<int> DeleteAsync(string key, string field, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["field"] = field };
        using var response = await _client.ExecuteAsync("hash.del", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("deleted", out var deleted) ? deleted.GetInt32() : 0;
    }

    /// <summary>
    /// Check if field exists in hash.
    /// </summary>
    public async Task<bool> ExistsAsync(string key, string field, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["field"] = field };
        using var response = await _client.ExecuteAsync("hash.exists", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("exists", out var exists) && exists.GetBoolean();
    }

    /// <summary>
    /// Get all field names in hash.
    /// </summary>
    public async Task<List<string>> KeysAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("hash.keys", key, null, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("fields", out var fields))
        {
            return JsonSerializer.Deserialize<List<string>>(fields.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Get all values in hash.
    /// </summary>
    public async Task<List<string>> ValuesAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("hash.values", key, null, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("values", out var values))
        {
            return JsonSerializer.Deserialize<List<string>>(values.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Get number of fields in hash.
    /// </summary>
    public async Task<int> LenAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("hash.len", key, null, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("length", out var length) ? length.GetInt32() : 0;
    }

    /// <summary>
    /// Set multiple fields in hash.
    /// </summary>
    public async Task<bool> MSetAsync(
        string key,
        Dictionary<string, string> fields,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["fields"] = fields };
        using var response = await _client.ExecuteAsync("hash.mset", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("success", out var success) && success.GetBoolean();
    }

    /// <summary>
    /// Get multiple fields from hash.
    /// </summary>
    public async Task<Dictionary<string, string?>> MGetAsync(
        string key,
        IReadOnlyList<string> fields,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["fields"] = fields };
        using var response = await _client.ExecuteAsync("hash.mget", key, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("values", out var values))
        {
            return JsonSerializer.Deserialize<Dictionary<string, string?>>(values.GetRawText()) ?? new();
        }

        return new Dictionary<string, string?>();
    }

    /// <summary>
    /// Increment field value by integer.
    /// </summary>
    public async Task<long> IncrByAsync(
        string key,
        string field,
        long increment,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["field"] = field,
            ["increment"] = increment
        };

        using var response = await _client.ExecuteAsync("hash.incrby", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("value", out var value) ? value.GetInt64() : 0;
    }

    /// <summary>
    /// Increment field value by float.
    /// </summary>
    public async Task<double> IncrByFloatAsync(
        string key,
        string field,
        double increment,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["field"] = field,
            ["increment"] = increment
        };

        using var response = await _client.ExecuteAsync("hash.incrbyfloat", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("value", out var value) ? value.GetDouble() : 0.0;
    }

    /// <summary>
    /// Set field only if it doesn't exist.
    /// </summary>
    public async Task<bool> SetNXAsync(
        string key,
        string field,
        string value,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["field"] = field,
            ["value"] = value
        };

        using var response = await _client.ExecuteAsync("hash.setnx", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("created", out var created) && created.GetBoolean();
    }
}

