using System.Text.Json;

namespace Synap.SDK.Modules;

/// <summary>
/// Set data structure operations (Redis-compatible).
/// </summary>
public sealed class SetManager
{
    private readonly SynapClient _client;

    internal SetManager(SynapClient client)
    {
        _client = client;
    }

    /// <summary>
    /// Add members to set.
    /// </summary>
    public async Task<int> AddAsync(
        string key,
        IReadOnlyList<string> members,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["members"] = members };
        using var response = await _client.ExecuteAsync("set.add", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("added", out var added) ? added.GetInt32() : 0;
    }

    /// <summary>
    /// Remove members from set.
    /// </summary>
    public async Task<int> RemAsync(
        string key,
        IReadOnlyList<string> members,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["members"] = members };
        using var response = await _client.ExecuteAsync("set.rem", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("removed", out var removed) ? removed.GetInt32() : 0;
    }

    /// <summary>
    /// Check if member exists in set.
    /// </summary>
    public async Task<bool> IsMemberAsync(
        string key,
        string member,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["member"] = member };
        using var response = await _client.ExecuteAsync("set.ismember", key, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("is_member", out var isMember) && isMember.GetBoolean();
    }

    /// <summary>
    /// Get all members of set.
    /// </summary>
    public async Task<List<string>> MembersAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("set.members", key, null, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("members", out var members))
        {
            return JsonSerializer.Deserialize<List<string>>(members.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Get set cardinality (size).
    /// </summary>
    public async Task<int> CardAsync(string key, CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("set.card", key, null, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("cardinality", out var cardinality) ? cardinality.GetInt32() : 0;
    }

    /// <summary>
    /// Remove and return random members.
    /// </summary>
    public async Task<List<string>> PopAsync(
        string key,
        int count = 1,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["count"] = count };
        using var response = await _client.ExecuteAsync("set.pop", key, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("members", out var members))
        {
            return JsonSerializer.Deserialize<List<string>>(members.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Get random members without removing.
    /// </summary>
    public async Task<List<string>> RandMemberAsync(
        string key,
        int count = 1,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["count"] = count };
        using var response = await _client.ExecuteAsync("set.randmember", key, data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("members", out var members))
        {
            return JsonSerializer.Deserialize<List<string>>(members.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Move member from source to destination set.
    /// </summary>
    public async Task<bool> MoveAsync(
        string source,
        string destination,
        string member,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["destination"] = destination,
            ["member"] = member
        };

        using var response = await _client.ExecuteAsync("set.move", source, data, cancellationToken).ConfigureAwait(false);

        return response.RootElement.TryGetProperty("moved", out var moved) && moved.GetBoolean();
    }

    /// <summary>
    /// Get intersection of sets.
    /// </summary>
    public async Task<List<string>> InterAsync(IReadOnlyList<string> keys, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["keys"] = keys };
        using var response = await _client.ExecuteAsync("set.inter", "", data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("members", out var members))
        {
            return JsonSerializer.Deserialize<List<string>>(members.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Get union of sets.
    /// </summary>
    public async Task<List<string>> UnionAsync(IReadOnlyList<string> keys, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["keys"] = keys };
        using var response = await _client.ExecuteAsync("set.union", "", data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("members", out var members))
        {
            return JsonSerializer.Deserialize<List<string>>(members.GetRawText()) ?? new();
        }

        return new List<string>();
    }

    /// <summary>
    /// Get difference of sets (first minus others).
    /// </summary>
    public async Task<List<string>> DiffAsync(IReadOnlyList<string> keys, CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["keys"] = keys };
        using var response = await _client.ExecuteAsync("set.diff", "", data, cancellationToken).ConfigureAwait(false);

        if (response.RootElement.TryGetProperty("members", out var members))
        {
            return JsonSerializer.Deserialize<List<string>>(members.GetRawText()) ?? new();
        }

        return new List<string>();
    }
}

