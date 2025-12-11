using System.Collections.ObjectModel;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Synap.SDK.Modules;

/// <summary>
/// Response from transaction operations.
/// </summary>
public class TransactionResponse
{
    [JsonPropertyName("success")]
    public bool Success { get; set; }

    [JsonPropertyName("message")]
    public string Message { get; set; } = string.Empty;
}

/// <summary>
/// Successful transaction execution result.
/// </summary>
public class TransactionExecSuccess
{
    [JsonPropertyName("success")]
    public bool Success { get; set; } = true;

    [JsonPropertyName("results")]
    public Collection<JsonElement> Results { get; } = new();
}

/// <summary>
/// Aborted transaction execution result.
/// </summary>
public class TransactionExecAborted
{
    [JsonPropertyName("success")]
    public bool Success { get; set; } = false;

    [JsonPropertyName("aborted")]
    public bool Aborted { get; set; } = true;

    [JsonPropertyName("message")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Message { get; set; }
}

/// <summary>
/// Transaction operations (Redis-compatible).
/// </summary>
public sealed class TransactionManager
{
    private readonly SynapClient _client;

    /// <summary>
    /// Initializes a new instance of the <see cref="TransactionManager"/> class.
    /// </summary>
    /// <param name="client">The Synap client instance.</param>
    public TransactionManager(SynapClient client)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
    }

    /// <summary>
    /// Start a transaction (MULTI).
    /// </summary>
    /// <param name="clientId">Optional client identifier to group commands within the same transaction.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Transaction response with success status and message.</returns>
    public async Task<TransactionResponse> MultiAsync(
        string? clientId = null,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>();
        if (!string.IsNullOrWhiteSpace(clientId))
        {
            data["client_id"] = clientId;
        }

        using var response = await _client.ExecuteAsync("transaction.multi", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload))
        {
            return JsonSerializer.Deserialize<TransactionResponse>(payload.GetRawText()) ?? new TransactionResponse
            {
                Success = true,
                Message = "Transaction started"
            };
        }

        return new TransactionResponse
        {
            Success = true,
            Message = "Transaction started"
        };
    }

    /// <summary>
    /// Discard the current transaction (DISCARD).
    /// </summary>
    /// <param name="clientId">Optional client identifier for the transaction.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Transaction response with success status and message.</returns>
    public async Task<TransactionResponse> DiscardAsync(
        string? clientId = null,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>();
        if (!string.IsNullOrWhiteSpace(clientId))
        {
            data["client_id"] = clientId;
        }

        using var response = await _client.ExecuteAsync("transaction.discard", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload))
        {
            return JsonSerializer.Deserialize<TransactionResponse>(payload.GetRawText()) ?? new TransactionResponse
            {
                Success = true,
                Message = "Transaction discarded"
            };
        }

        return new TransactionResponse
        {
            Success = true,
            Message = "Transaction discarded"
        };
    }

    /// <summary>
    /// Watch keys for optimistic locking (WATCH).
    /// </summary>
    /// <param name="keys">Collection of keys to watch for changes.</param>
    /// <param name="clientId">Optional client identifier for the transaction.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Transaction response with success status and message.</returns>
    /// <exception cref="ArgumentException">Thrown when keys collection is empty.</exception>
    public async Task<TransactionResponse> WatchAsync(
        IReadOnlyCollection<string> keys,
        string? clientId = null,
        CancellationToken cancellationToken = default)
    {
        if (keys == null || keys.Count == 0)
        {
            throw new ArgumentException("Transaction watch requires at least one key", nameof(keys));
        }

        var data = new Dictionary<string, object?>
        {
            ["keys"] = keys
        };
        if (!string.IsNullOrWhiteSpace(clientId))
        {
            data["client_id"] = clientId;
        }

        using var response = await _client.ExecuteAsync("transaction.watch", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload))
        {
            return JsonSerializer.Deserialize<TransactionResponse>(payload.GetRawText()) ?? new TransactionResponse
            {
                Success = true,
                Message = "Keys watched"
            };
        }

        return new TransactionResponse
        {
            Success = true,
            Message = "Keys watched"
        };
    }

    /// <summary>
    /// Remove all watched keys (UNWATCH).
    /// </summary>
    /// <param name="clientId">Optional client identifier for the transaction.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Transaction response with success status and message.</returns>
    public async Task<TransactionResponse> UnwatchAsync(
        string? clientId = null,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>();
        if (!string.IsNullOrWhiteSpace(clientId))
        {
            data["client_id"] = clientId;
        }

        using var response = await _client.ExecuteAsync("transaction.unwatch", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload))
        {
            return JsonSerializer.Deserialize<TransactionResponse>(payload.GetRawText()) ?? new TransactionResponse
            {
                Success = true,
                Message = "Keys unwatched"
            };
        }

        return new TransactionResponse
        {
            Success = true,
            Message = "Keys unwatched"
        };
    }

    /// <summary>
    /// Execute queued commands (EXEC).
    /// </summary>
    /// <param name="clientId">Optional client identifier for the transaction.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Transaction execution result. If successful, returns results array. If aborted (due to watched keys changed), returns aborted status.</returns>
    public async Task<object> ExecAsync(
        string? clientId = null,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>();
        if (!string.IsNullOrWhiteSpace(clientId))
        {
            data["client_id"] = clientId;
        }

        using var response = await _client.ExecuteAsync("transaction.exec", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload))
        {
            if (payload.TryGetProperty("results", out var results))
            {
                var successResult = new TransactionExecSuccess();
                if (results.ValueKind == JsonValueKind.Array)
                {
                    foreach (var item in results.EnumerateArray())
                    {
                        successResult.Results.Add(item);
                    }
                }
                return successResult;
            }

            if (payload.TryGetProperty("aborted", out _))
            {
                return JsonSerializer.Deserialize<TransactionExecAborted>(payload.GetRawText()) ?? new TransactionExecAborted();
            }
        }

        return new TransactionExecAborted();
    }
}

