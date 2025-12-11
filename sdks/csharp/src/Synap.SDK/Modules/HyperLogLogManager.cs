using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Synap.SDK.Modules;

/// <summary>
/// HyperLogLog operations (Redis-compatible).
/// </summary>
public sealed class HyperLogLogManager
{
    private readonly SynapClient _client;

    public HyperLogLogManager(SynapClient client)
    {
        _client = client;
    }

    /// <summary>
    /// Add elements to a HyperLogLog structure (PFADD).
    /// </summary>
    /// <param name="key">HyperLogLog key</param>
    /// <param name="elements">Elements to add (strings or byte arrays)</param>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>Number of elements added (approximate)</returns>
    public async Task<int> PfAddAsync(
        string key,
        IEnumerable<string> elements,
        CancellationToken cancellationToken = default)
    {
        var elementList = elements.ToList();
        if (elementList.Count == 0)
        {
            return 0;
        }

        // Encode elements to byte arrays
        var encoded = elementList.Select(e => Encoding.UTF8.GetBytes(e).Select(b => (int)b).ToArray()).ToList();

        var data = new Dictionary<string, object?>
        {
            ["key"] = key,
            ["elements"] = encoded
        };

        using var response = await _client.ExecuteAsync("hyperloglog.pfadd", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("added", out var added))
        {
            return added.GetInt32();
        }
        return 0;
    }

    /// <summary>
    /// Estimate cardinality of a HyperLogLog structure (PFCOUNT).
    /// </summary>
    /// <param name="key">HyperLogLog key</param>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>Estimated cardinality (approximate count)</returns>
    public async Task<long> PfCountAsync(
        string key,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["key"] = key };
        using var response = await _client.ExecuteAsync("hyperloglog.pfcount", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("count", out var count))
        {
            return count.GetInt64();
        }
        return 0;
    }

    /// <summary>
    /// Merge multiple HyperLogLog structures (PFMERGE).
    /// </summary>
    /// <param name="destination">Destination key for merged result</param>
    /// <param name="sources">Source HyperLogLog keys to merge</param>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>Estimated cardinality of merged result</returns>
    public async Task<long> PfMergeAsync(
        string destination,
        IReadOnlyList<string> sources,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(sources);

        if (sources.Count == 0)
        {
            throw new ArgumentException("PFMERGE requires at least one source key", nameof(sources));
        }

        var data = new Dictionary<string, object?>
        {
            ["destination"] = destination,
            ["sources"] = sources
        };

        using var response = await _client.ExecuteAsync("hyperloglog.pfmerge", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("count", out var count))
        {
            return count.GetInt64();
        }
        return 0;
    }

    /// <summary>
    /// Retrieve HyperLogLog statistics.
    /// </summary>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>HyperLogLog statistics</returns>
    public async Task<HyperLogLogStats> StatsAsync(CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("hyperloglog.stats", string.Empty, null, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload))
        {
            return JsonSerializer.Deserialize<HyperLogLogStats>(payload.GetRawText()) ?? new HyperLogLogStats();
        }
        return new HyperLogLogStats();
    }
}

/// <summary>
/// HyperLogLog statistics.
/// </summary>
public sealed class HyperLogLogStats
{
    /// <summary>
    /// Total number of HyperLogLog structures.
    /// </summary>
    [JsonPropertyName("total_hlls")]
    public long TotalHlls { get; set; }

    /// <summary>
    /// Total estimated cardinality across all HyperLogLog structures.
    /// </summary>
    [JsonPropertyName("total_cardinality")]
    public long TotalCardinality { get; set; }

    /// <summary>
    /// Number of PFADD operations performed.
    /// </summary>
    [JsonPropertyName("pfadd_count")]
    public long PfAddCount { get; set; }

    /// <summary>
    /// Number of PFCOUNT operations performed.
    /// </summary>
    [JsonPropertyName("pfcount_count")]
    public long PfCountCount { get; set; }

    /// <summary>
    /// Number of PFMERGE operations performed.
    /// </summary>
    [JsonPropertyName("pfmerge_count")]
    public long PfMergeCount { get; set; }
}

