using System.Net.Http.Json;
using System.Text.Json;
using Synap.SDK.Exceptions;
using Synap.SDK.Modules;
using Synap.SDK.Transports;

namespace Synap.SDK;

/// <summary>
/// Main Synap SDK client for interacting with the Synap server.
///
/// By default, the client connects over SynapRPC (MessagePack over TCP, port 15501).
/// Use the URL scheme in <see cref="SynapConfig"/> to select a transport:
/// <list type="bullet">
///   <item><c>synap://host:port</c> — SynapRPC (default)</item>
///   <item><c>resp3://host:port</c> — RESP3</item>
///   <item><c>http://host:port</c>  — HTTP</item>
/// </list>
/// </summary>
public sealed class SynapClient : IDisposable
{
    private readonly HttpClient _httpClient;
    private readonly SynapConfig _config;
    private readonly bool _disposeHttpClient;

    // Active native transport (null when using HTTP).
    private readonly ITransport? _transport;

    // Cached module instances (lazy-initialised on first access).
    private KVStore? _kv;
    private HashManager? _hash;
    private ListManager? _list;
    private SetManager? _set;
    private QueueManager? _queue;
    private StreamManager? _stream;
    private PubSubManager? _pubsub;
    private BitmapManager? _bitmap;
    private HyperLogLogManager? _hyperloglog;
    private GeospatialManager? _geospatial;
    private TransactionManager? _transaction;

    /// <summary>
    /// Initializes a new instance of the <see cref="SynapClient"/> class with the default
    /// SynapRPC transport (<c>synap://127.0.0.1:15501</c>).
    /// </summary>
    public SynapClient() : this(new SynapConfig())
    {
    }

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
    /// <param name="httpClient">
    /// Optional custom HTTP client. When provided, native transports are disabled so that
    /// tests can inject a mocked HttpClient without opening TCP connections.
    /// </param>
    public SynapClient(SynapConfig config, HttpClient? httpClient)
    {
        _config = config ?? throw new ArgumentNullException(nameof(config));

        if (httpClient is not null)
        {
            // Custom client supplied (usually for testing) — use HTTP only.
            _httpClient = httpClient;
            _disposeHttpClient = false;
        }
        else
        {
            _httpClient = new HttpClient
            {
                BaseAddress = new Uri(config.BaseUrl),
                Timeout = TimeSpan.FromSeconds(config.Timeout),
            };
            _disposeHttpClient = true;

            // Instantiate native transport based on configured mode.
            _transport = config.Transport switch
            {
                TransportMode.SynapRpc => new SynapRpcTransport(config.RpcHost, config.RpcPort, config.Timeout),
                TransportMode.Resp3    => new Resp3Transport(config.Resp3Host, config.Resp3Port, config.Timeout),
                _                      => null,
            };
        }

        _httpClient.DefaultRequestHeaders.Add("Accept", "application/json");

        if (!string.IsNullOrWhiteSpace(config.AuthToken))
        {
            _httpClient.DefaultRequestHeaders.Authorization =
                new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", config.AuthToken);
        }
        else if (!string.IsNullOrWhiteSpace(config.Username) && !string.IsNullOrWhiteSpace(config.Password))
        {
            var credentials = Convert.ToBase64String(
                System.Text.Encoding.UTF8.GetBytes($"{config.Username}:{config.Password}"));
            _httpClient.DefaultRequestHeaders.Authorization =
                new System.Net.Http.Headers.AuthenticationHeaderValue("Basic", credentials);
        }
    }

    /// <summary>Gets the Key-Value Store operations.</summary>
    public KVStore KV => _kv ??= new KVStore(this);

    /// <summary>Gets the Hash data structure operations.</summary>
    public HashManager Hash => _hash ??= new HashManager(this);

    /// <summary>Gets the List data structure operations.</summary>
    public ListManager List => _list ??= new ListManager(this);

    /// <summary>Gets the Set data structure operations.</summary>
    public SetManager Set => _set ??= new SetManager(this);

    /// <summary>Gets the Queue operations.</summary>
    public QueueManager Queue => _queue ??= new QueueManager(this);

    /// <summary>Gets the Stream operations.</summary>
    public StreamManager Stream => _stream ??= new StreamManager(this);

    /// <summary>Gets the Pub/Sub operations.</summary>
    public PubSubManager PubSub => _pubsub ??= new PubSubManager(this);

    /// <summary>Gets the Bitmap operations.</summary>
    public BitmapManager Bitmap => _bitmap ??= new BitmapManager(this);

    /// <summary>Gets the HyperLogLog operations.</summary>
    public HyperLogLogManager HyperLogLog => _hyperloglog ??= new HyperLogLogManager(this);

    /// <summary>Gets the Geospatial operations.</summary>
    public GeospatialManager Geospatial => _geospatial ??= new GeospatialManager(this);

    /// <summary>Gets the Transaction operations.</summary>
    public TransactionManager Transaction => _transaction ??= new TransactionManager(this);

    /// <summary>Gets the client configuration.</summary>
    public SynapConfig Config => _config;

    /// <summary>
    /// Executes a Synap operation, routing through native transport when available.
    /// Falls back to HTTP for operations that are not mapped to native commands
    /// (e.g. queues, streams, pub/sub).
    /// </summary>
    /// <param name="operation">The operation type (e.g., 'kv.set', 'queue.publish').</param>
    /// <param name="target">The target resource (key name, queue name, etc.).</param>
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
        ArgumentNullException.ThrowIfNull(operation);

        // Assemble payload (merge target into the data dict as "key" or "destination").
        var payloadData = data is not null
            ? new Dictionary<string, object?>(data)
            : new Dictionary<string, object?>();

        if (!string.IsNullOrEmpty(target))
        {
            if (operation.StartsWith("bitmap.bitop", StringComparison.Ordinal) ||
                operation.StartsWith("hyperloglog.pfmerge", StringComparison.Ordinal))
            {
                payloadData["destination"] = target;
            }
            else
            {
                payloadData["key"] = target;
            }
        }

        // Try native transport for mapped commands.
        if (_transport is not null)
        {
            var mapped = CommandMapper.MapCommand(operation, payloadData);
            if (mapped.HasValue)
            {
                var (cmd, args) = mapped.Value;
                try
                {
                    var raw = await _transport.ExecuteAsync(cmd, args, cancellationToken).ConfigureAwait(false);
                    var responseDict = CommandMapper.MapResponse(operation, raw);
                    return DictToJsonDocument(responseDict);
                }
                catch (SynapException)
                {
                    throw;
                }
                catch (Exception ex)
                {
                    var transportName = _config.Transport == TransportMode.Resp3 ? "resp3" : "synaprpc";
                    throw SynapException.NetworkError($"{transportName} error: {ex.Message}");
                }
            }

            // Command has no native mapping — raise instead of silently falling back.
            var transportLabel = _config.Transport == TransportMode.Resp3 ? "resp3" : "synaprpc";
            throw new UnsupportedCommandException(operation, transportLabel);
        }

        // HTTP path.
        return await ExecuteHttpAsync(operation, payloadData, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Sends a command with a pre-assembled payload dictionary (preferred over ExecuteAsync).
    ///
    /// Mapped commands are routed through native transport (SynapRPC or RESP3).
    /// Unmapped commands raise <see cref="UnsupportedCommandException"/> on native transports.
    /// </summary>
    /// <param name="command">Command name (e.g. "queue.publish").</param>
    /// <param name="payload">Full payload — no target injection.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>The response as a JSON document.</returns>
    public async Task<JsonDocument> SendCommandAsync(
        string command,
        Dictionary<string, object?>? payload = null,
        CancellationToken cancellationToken = default)
    {
        var pl = payload ?? new Dictionary<string, object?>();

        if (_transport is not null)
        {
            var transportLabel = _config.Transport == TransportMode.Resp3 ? "resp3" : "synaprpc";
            var mapped = CommandMapper.MapCommand(command, pl);
            if (mapped.HasValue)
            {
                var (cmd, args) = mapped.Value;
                try
                {
                    var raw = await _transport.ExecuteAsync(cmd, args, cancellationToken).ConfigureAwait(false);
                    return DictToJsonDocument(CommandMapper.MapResponse(command, raw));
                }
                catch (SynapException) { throw; }
                catch (Exception ex) { throw SynapException.NetworkError($"{transportLabel} error: {ex.Message}"); }
            }
            throw new UnsupportedCommandException(command, transportLabel);
        }

        return await ExecuteHttpAsync(command, pl, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Returns the active <see cref="SynapRpcTransport"/> if the client was configured
    /// with a SynapRPC transport, or <c>null</c> otherwise.
    /// </summary>
    internal SynapRpcTransport? GetSynapRpcTransport() => _transport as SynapRpcTransport;

    private async Task<JsonDocument> ExecuteHttpAsync(
        string operation,
        Dictionary<string, object?> payloadData,
        CancellationToken cancellationToken)
    {
        try
        {
            var payload = new Dictionary<string, object?>
            {
                ["command"] = operation,
                ["payload"] = payloadData,
                ["request_id"] = Guid.NewGuid().ToString(),
            };

            var response = await _httpClient.PostAsJsonAsync(
                "/api/v1/command",
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

            if (!response.IsSuccessStatusCode)
            {
                throw SynapException.HttpError(
                    $"Request failed with status {response.StatusCode}",
                    (int)response.StatusCode);
            }

            if (result.RootElement.TryGetProperty("success", out var successElement))
            {
                if (!successElement.GetBoolean())
                {
                    var errorMessage = "Unknown error";
                    if (result.RootElement.TryGetProperty("error", out var errorElement))
                    {
                        errorMessage = errorElement.GetString() ?? errorMessage;
                    }

                    throw SynapException.ServerError(errorMessage);
                }
            }

            return result;
        }
        catch (HttpRequestException ex)
        {
            throw SynapException.NetworkError(ex.Message);
        }
        catch (TaskCanceledException ex) when (ex.CancellationToken == cancellationToken)
        {
            throw;
        }
        catch (TaskCanceledException ex)
        {
            throw SynapException.NetworkError($"Request timed out: {ex.Message}");
        }
    }

    private static JsonDocument DictToJsonDocument(Dictionary<string, object?> dict)
    {
        var json = JsonSerializer.Serialize(dict);
        return JsonDocument.Parse(json);
    }

    /// <summary>Disposes the client and releases resources.</summary>
    public void Dispose()
    {
        _transport?.Dispose();

        if (_disposeHttpClient)
        {
            _httpClient.Dispose();
        }
    }
}
