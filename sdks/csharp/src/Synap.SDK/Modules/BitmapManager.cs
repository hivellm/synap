using System.Text.Json;
using System.Text.Json.Serialization;

namespace Synap.SDK.Modules;

/// <summary>
/// Bitmap operations (Redis-compatible).
/// </summary>
public sealed class BitmapManager
{
    private readonly SynapClient _client;

    public BitmapManager(SynapClient client)
    {
        _client = client;
    }

    /// <summary>
    /// Bitmap operation types.
    /// </summary>
    public enum BitmapOperation
    {
        AND,
        OR,
        XOR,
        NOT
    }

    /// <summary>
    /// Set bit at offset to value (SETBIT).
    /// </summary>
    /// <param name="key">Bitmap key</param>
    /// <param name="offset">Bit offset (0-based)</param>
    /// <param name="value">Bit value (0 or 1)</param>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>Previous bit value (0 or 1)</returns>
    public async Task<byte> SetBitAsync(
        string key,
        int offset,
        byte value,
        CancellationToken cancellationToken = default)
    {
        if (value > 1)
        {
            throw new ArgumentException("Bitmap value must be 0 or 1", nameof(value));
        }

        var data = new Dictionary<string, object?>
        {
            ["key"] = key,
            ["offset"] = offset,
            ["value"] = value
        };

        using var response = await _client.ExecuteAsync("bitmap.setbit", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("old_value", out var oldValue))
        {
            return oldValue.GetByte();
        }
        return (byte)0;
    }

    /// <summary>
    /// Get bit at offset (GETBIT).
    /// </summary>
    /// <param name="key">Bitmap key</param>
    /// <param name="offset">Bit offset (0-based)</param>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>Bit value (0 or 1)</returns>
    public async Task<byte> GetBitAsync(
        string key,
        int offset,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["key"] = key, ["offset"] = offset };
        using var response = await _client.ExecuteAsync("bitmap.getbit", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("value", out var value))
        {
            return value.GetByte();
        }
        return (byte)0;
    }

    /// <summary>
    /// Count set bits in bitmap (BITCOUNT).
    /// </summary>
    /// <param name="key">Bitmap key</param>
    /// <param name="start">Optional start offset (inclusive)</param>
    /// <param name="end">Optional end offset (inclusive)</param>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>Number of set bits</returns>
    public async Task<int> BitCountAsync(
        string key,
        int? start = null,
        int? end = null,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?> { ["key"] = key };
        if (start.HasValue)
        {
            data["start"] = start.Value;
        }
        if (end.HasValue)
        {
            data["end"] = end.Value;
        }

        using var response = await _client.ExecuteAsync("bitmap.bitcount", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("count", out var count))
        {
            return count.GetInt32();
        }
        return 0;
    }

    /// <summary>
    /// Find first bit set to value (BITPOS).
    /// </summary>
    /// <param name="key">Bitmap key</param>
    /// <param name="value">Bit value to search for (0 or 1)</param>
    /// <param name="start">Optional start offset (inclusive)</param>
    /// <param name="end">Optional end offset (inclusive)</param>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>Position of first matching bit, or null if not found</returns>
    public async Task<int?> BitPosAsync(
        string key,
        byte value,
        int? start = null,
        int? end = null,
        CancellationToken cancellationToken = default)
    {
        if (value > 1)
        {
            throw new ArgumentException("Bitmap value must be 0 or 1", nameof(value));
        }

        var data = new Dictionary<string, object?> { ["key"] = key, ["value"] = value };
        if (start.HasValue)
        {
            data["start"] = start.Value;
        }
        if (end.HasValue)
        {
            data["end"] = end.Value;
        }

        using var response = await _client.ExecuteAsync("bitmap.bitpos", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("position", out var position))
        {
            if (position.ValueKind == JsonValueKind.Null)
            {
                return null;
            }
            return position.GetInt32();
        }
        return null;
    }

    /// <summary>
    /// Perform bitwise operation on multiple bitmaps (BITOP).
    /// </summary>
    /// <param name="operation">Bitwise operation (AND, OR, XOR, NOT)</param>
    /// <param name="destination">Destination key for result</param>
    /// <param name="sourceKeys">Source bitmap keys (NOT requires exactly 1 source)</param>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>Length of resulting bitmap in bits</returns>
    public async Task<int> BitOpAsync(
        BitmapOperation operation,
        string destination,
        IReadOnlyList<string> sourceKeys,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(sourceKeys);

        if (operation == BitmapOperation.NOT && sourceKeys.Count != 1)
        {
            throw new ArgumentException("NOT operation requires exactly one source key", nameof(sourceKeys));
        }

        if (sourceKeys.Count == 0)
        {
            throw new ArgumentException("BITOP requires at least one source key", nameof(sourceKeys));
        }

        var data = new Dictionary<string, object?>
        {
            ["destination"] = destination,
            ["operation"] = operation.ToString(),
            ["source_keys"] = sourceKeys
        };

        using var response = await _client.ExecuteAsync("bitmap.bitop", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("length", out var length))
        {
            return length.GetInt32();
        }
        return 0;
    }

    /// <summary>
    /// Bitfield operation type.
    /// </summary>
    public enum BitfieldOperationType
    {
        GET,
        SET,
        INCRBY
    }

    /// <summary>
    /// Bitfield overflow behavior.
    /// </summary>
    public enum BitfieldOverflow
    {
        WRAP,
        SAT,
        FAIL
    }


    /// <summary>
    /// Execute bitfield operations (BITFIELD).
    /// </summary>
    /// <param name="key">Bitmap key</param>
    /// <param name="operations">List of bitfield operations</param>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>List of result values (one per operation)</returns>
    public async Task<List<long>> BitFieldAsync(
        string key,
        IReadOnlyList<BitfieldOperation> operations,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(operations);

        var operationsJson = operations.Select(op => new Dictionary<string, object?>
        {
            ["operation"] = op.Operation.ToString(),
            ["offset"] = op.Offset,
            ["width"] = op.Width,
            ["signed"] = op.IsSigned,
            ["value"] = op.Value,
            ["increment"] = op.Increment,
            ["overflow"] = op.Overflow?.ToString(),
        }).ToList();

        var data = new Dictionary<string, object?>
        {
            ["key"] = key,
            ["operations"] = operationsJson
        };

        using var response = await _client.ExecuteAsync("bitmap.bitfield", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("results", out var results))
        {
            var resultList = new List<long>();
            foreach (var result in results.EnumerateArray())
            {
                resultList.Add(result.GetInt64());
            }
            return resultList;
        }
        return new List<long>();
    }

    /// <summary>
    /// Retrieve bitmap statistics.
    /// </summary>
    /// <param name="cancellationToken">Cancellation token</param>
    /// <returns>Bitmap statistics</returns>
    public async Task<BitmapStats> StatsAsync(CancellationToken cancellationToken = default)
    {
        using var response = await _client.ExecuteAsync("bitmap.stats", string.Empty, null, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload))
        {
            return JsonSerializer.Deserialize<BitmapStats>(payload.GetRawText()) ?? new BitmapStats();
        }
        return new BitmapStats();
    }
}

/// <summary>
/// Bitmap statistics.
/// </summary>
public sealed class BitmapStats
{
    /// <summary>
    /// Total number of bitmaps.
    /// </summary>
    [JsonPropertyName("total_bitmaps")]
    public int TotalBitmaps { get; set; }

    /// <summary>
    /// Total number of bits across all bitmaps.
    /// </summary>
    [JsonPropertyName("total_bits")]
    public long TotalBits { get; set; }

    /// <summary>
    /// Number of SETBIT operations performed.
    /// </summary>
    [JsonPropertyName("setbit_count")]
    public long SetBitCount { get; set; }

    /// <summary>
    /// Number of GETBIT operations performed.
    /// </summary>
    [JsonPropertyName("getbit_count")]
    public long GetBitCount { get; set; }

    /// <summary>
    /// Number of BITCOUNT operations performed.
    /// </summary>
    [JsonPropertyName("bitcount_count")]
    public long BitCountCount { get; set; }

    /// <summary>
    /// Number of BITOP operations performed.
    /// </summary>
    [JsonPropertyName("bitop_count")]
    public long BitOpCount { get; set; }

    /// <summary>
    /// Number of BITPOS operations performed.
    /// </summary>
    [JsonPropertyName("bitpos_count")]
    public long BitPosCount { get; set; }

    /// <summary>
    /// Number of BITFIELD operations performed.
    /// </summary>
    [JsonPropertyName("bitfield_count")]
    public long BitFieldCount { get; set; }
}

