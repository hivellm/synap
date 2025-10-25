using System.Net.Http.Json;
using System.Text.Json;
using Synap.SDK.Exceptions;
using Synap.SDK.Modules;

namespace Synap.SDK;

/// <summary>
/// Main Synap SDK client for interacting with the Synap server.
/// </summary>
public sealed class SynapClient : IDisposable
{
    private readonly HttpClient _httpClient;
    private readonly SynapConfig _config;
    private readonly bool _disposeHttpClient;

    private KVStore? _kv;
    private HashManager? _hash;
    private ListManager? _list;
    private SetManager? _set;
    private QueueManager? _queue;
    private StreamManager? _stream;
    private PubSubManager? _pubsub;

    /// <summary>
    /// Initializes a new instance of the <see cref="SynapClient"/> class.
    /// </summary>
    /// <param name="config">The client configuration.</param>
    public SynapClient(SynapConfig config) : this(config, null)
    {
    }

    /// <summary>
    /// Initializes a new instance of the <see cref="SynapClient"/> class.
    /// </summary>
    /// <param name="config">The client configuration.</param>
    /// <param name="httpClient">Optional custom HTTP client. If null, a new one will be created.</param>
    public SynapClient(SynapConfig config, HttpClient? httpClient)
    {
        _config = config ?? throw new ArgumentNullException(nameof(config));

        if (httpClient is not null)
        {
            _httpClient = httpClient;
            _disposeHttpClient = false;
        }
        else
        {
            _httpClient = new HttpClient
            {
                BaseAddress = new Uri(config.BaseUrl),
                Timeout = TimeSpan.FromSeconds(config.Timeout)
            };
            _disposeHttpClient = true;
        }

        _httpClient.DefaultRequestHeaders.Add("Accept", "application/json");

        if (!string.IsNullOrWhiteSpace(config.AuthToken))
        {
            _httpClient.DefaultRequestHeaders.Authorization =
                new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", config.AuthToken);
        }
    }

    /// <summary>
    /// Gets the Key-Value Store operations.
    /// </summary>
    public KVStore KV => _kv ??= new KVStore(this);

    /// <summary>
    /// Gets the Hash data structure operations.
    /// </summary>
    public HashManager Hash => _hash ??= new HashManager(this);

    /// <summary>
    /// Gets the List data structure operations.
    /// </summary>
    public ListManager List => _list ??= new ListManager(this);

    /// <summary>
    /// Gets the Set data structure operations.
    /// </summary>
    public SetManager Set => _set ??= new SetManager(this);

    /// <summary>
    /// Gets the Queue operations.
    /// </summary>
    public QueueManager Queue => _queue ??= new QueueManager(this);

    /// <summary>
    /// Gets the Stream operations.
    /// </summary>
    public StreamManager Stream => _stream ??= new StreamManager(this);

    /// <summary>
    /// Gets the Pub/Sub operations.
    /// </summary>
    public PubSubManager PubSub => _pubsub ??= new PubSubManager(this);

    /// <summary>
    /// Gets the client configuration.
    /// </summary>
    public SynapConfig Config => _config;

    /// <summary>
    /// Executes a StreamableHTTP operation on the Synap server.
    /// </summary>
    /// <param name="operation">The operation type (e.g., 'kv.set', 'queue.publish').</param>
    /// <param name="target">The target resource (e.g., key name, queue name).</param>
    /// <param name="data">The operation data.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The response as a JSON document.</returns>
    /// <exception cref="SynapException">Thrown when the operation fails.</exception>
    public async Task<JsonDocument> ExecuteAsync(
        string operation,
        string target,
        Dictionary<string, object?>? data = null,
        CancellationToken cancellationToken = default)
    {
        try
        {
            var payload = new Dictionary<string, object?>
            {
                ["operation"] = operation,
                ["target"] = target,
                ["data"] = data ?? new Dictionary<string, object?>()
            };

            var response = await _httpClient.PostAsJsonAsync(
                "/api/stream",
                payload,
                cancellationToken).ConfigureAwait(false);

            var content = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);

            if (string.IsNullOrWhiteSpace(content))
            {
                return JsonDocument.Parse("{}");
            }

            JsonDocument result;
            try
            {
                result = JsonDocument.Parse(content);
            }
            catch (JsonException ex)
            {
                throw SynapException.InvalidResponse($"Failed to parse JSON response: {ex.Message}");
            }

            // Check for server error in response
            if (result.RootElement.TryGetProperty("error", out var errorElement))
            {
                var errorMessage = errorElement.GetString() ?? "Unknown error";
                throw SynapException.ServerError(errorMessage);
            }

            if (!response.IsSuccessStatusCode)
            {
                throw SynapException.HttpError(
                    $"Request failed with status {response.StatusCode}",
                    (int)response.StatusCode);
            }

            return result;
        }
        catch (HttpRequestException ex)
        {
            throw SynapException.NetworkError(ex.Message);
        }
        catch (TaskCanceledException ex) when (ex.CancellationToken == cancellationToken)
        {
            throw; // Rethrow if it was our cancellation token
        }
        catch (TaskCanceledException ex)
        {
            throw SynapException.NetworkError($"Request timed out: {ex.Message}");
        }
    }

    /// <summary>
    /// Disposes the client and releases resources.
    /// </summary>
    public void Dispose()
    {
        if (_disposeHttpClient)
        {
            _httpClient.Dispose();
        }
    }
}

