using System.Runtime.CompilerServices;
using System.Threading.Channels;
using HiveLLM.Thunder;
using Synap.SDK.Exceptions;
using Synap.SDK.Transports;

namespace Synap.SDK;

/// <summary>
/// SynapRPC transport, backed by <see href="https://github.com/hivellm/thunder">Thunder</see>.
/// </summary>
/// <remarks>
/// <para>
/// The wire layer is not implemented here. <c>HiveLLM.Thunder</c> is the HiveLLM
/// family's shared binary RPC client — the same protocol the Synap server runs
/// on, so the two ends of the wire cannot drift.
/// </para>
/// <para>
/// What Thunder brings that the hand-written transport did not: the frame cap
/// validated against the length prefix <i>before</i> allocating (this transport
/// called <c>new byte[msgLen]</c> with whatever a remote peer's four bytes
/// claimed); a real handshake, so the SDK can authenticate on the RPC port; and
/// a push hook for <c>SUBSCRIBE</c>. It also removes the SDK's hand-rolled
/// MessagePack encoder.
/// </para>
/// </remarks>
internal sealed class SynapRpcTransport : ITransport
{
    /// <summary>Frame-body cap, matching the server's <c>MAX_FRAME_BYTES</c>.</summary>
    internal const int MaxFrameBytes = 512 * 1024 * 1024;

    /// <summary>Default SynapRPC port.</summary>
    internal const int DefaultRpcPort = 15501;

    private readonly string _endpoint;
    private readonly ClientConfig _clientConfig;
    private readonly SemaphoreSlim _connectLock = new(1, 1);

    private ThunderClient? _client;
    private bool _disposed;

    internal SynapRpcTransport(string host, int port, int timeoutSeconds, Credentials? credentials = null)
    {
        _endpoint = $"synap://{host}:{port}";
        var timeout = TimeSpan.FromSeconds(timeoutSeconds);
        _clientConfig = new ClientConfig
        {
            ConnectTimeout = timeout,
            CallTimeout = timeout,
            Credentials = credentials,
            ClientName = "synap-csharp-sdk",
        };
    }

    /// <summary>
    /// How Synap uses the Thunder wire, mirroring the server's <c>synap_config()</c>.
    /// </summary>
    /// <remarks>
    /// Thunder ships one standard and zero product knowledge, so this
    /// description lives in Synap's own repository. Every divergence from the
    /// standard is explicit: Synap authenticates with <c>AUTH</c> rather than a
    /// mandatory <c>HELLO</c>, it ships a push-producing command
    /// (<c>SUBSCRIBE</c>), its errors use the Redis-compatible prefixes it
    /// shares with its RESP3 port, and its frame cap is 512 MiB rather than 64.
    /// </remarks>
    internal static Config SynapConfig() => Config.Standard() with
    {
        Scheme = "synap",
        DefaultPort = DefaultRpcPort,
        Handshake = Handshake.AuthCommand,
        HelloStyle = HelloStyle.NotUsed,
        Push = PushPolicy.Enabled,
        ErrorCodes = ErrorConvention.Resp3Prefixes,
        MaxFrameBytes = MaxFrameBytes,
    };

    /// <summary>Dial a fresh Thunder client against the configured endpoint.</summary>
    private async Task<ThunderClient> DialAsync(CancellationToken ct)
    {
        try
        {
            return await ThunderClient.ConnectAsync(_endpoint, SynapConfig(), _clientConfig, ct)
                .ConfigureAwait(false);
        }
        catch (ThunderException ex)
        {
            throw ToSynapException(ex);
        }
    }

    /// <summary>The shared client, dialed on first use.</summary>
    private async Task<ThunderClient> EnsureConnectedAsync(CancellationToken ct)
    {
        var existing = _client;
        if (existing is not null)
        {
            return existing;
        }

        await _connectLock.WaitAsync(ct).ConfigureAwait(false);
        try
        {
            _client ??= await DialAsync(ct).ConfigureAwait(false);
            return _client;
        }
        finally
        {
            _connectLock.Release();
        }
    }

    /// <summary>
    /// Execute a command and return the decoded response.
    /// </summary>
    /// <remarks>
    /// Concurrent callers multiplex over the one connection, demultiplexed by
    /// frame id; the demultiplexer, timeouts and reconnect all come from Thunder.
    /// </remarks>
    public async Task<object?> ExecuteAsync(string command, object?[] args, CancellationToken ct = default)
    {
        ArgumentNullException.ThrowIfNull(command);
        ArgumentNullException.ThrowIfNull(args);

        var client = await EnsureConnectedAsync(ct).ConfigureAwait(false);

        var wireArgs = new Value[args.Length];
        for (var i = 0; i < args.Length; i++)
        {
            wireArgs[i] = WireValue.ToWire(args[i]);
        }

        try
        {
            var result = await client.CallAsync(command.ToUpperInvariant(), wireArgs, ct)
                .ConfigureAwait(false);
            return WireValue.FromWire(result);
        }
        catch (ThunderException ex)
        {
            throw ToSynapException(ex);
        }
    }

    /// <summary>
    /// Opens a dedicated server-push connection, sends SUBSCRIBE, and yields
    /// push messages as an async stream.
    /// </summary>
    /// <remarks>
    /// <para>
    /// The push hook is registered before SUBSCRIBE is sent, so a message
    /// published between the server's acknowledgement and the reader starting
    /// cannot be lost.
    /// </para>
    /// <para>
    /// The previous implementation sent SUBSCRIBE with <c>id = 0xFFFFFFFF</c> —
    /// the reserved push sentinel. A Thunder server refuses a request carrying
    /// that id, so the old frame would have been rejected outright. Thunder
    /// allocates a normal request id and routes push frames by the sentinel,
    /// which is what the sentinel is for.
    /// </para>
    /// </remarks>
    /// <param name="topics">Topic patterns to subscribe to.</param>
    /// <param name="cancellationToken">Token used to stop the stream.</param>
    /// <returns>An async stream of push-message dictionaries.</returns>
    internal async IAsyncEnumerable<Dictionary<string, object?>> SubscribePushAsync(
        IEnumerable<string> topics,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(topics);

        var client = await DialAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            // Unbounded is safe here: the producer is one connection's reader
            // task, and the consumer is the caller's `await foreach`.
            var channel = Channel.CreateUnbounded<Dictionary<string, object?>>(
                new UnboundedChannelOptions { SingleReader = true, SingleWriter = true });

            client.OnPush(value =>
            {
                var topic = value.MapGet("topic")?.AsStr();
                if (topic is null)
                {
                    return;
                }

                channel.Writer.TryWrite(new Dictionary<string, object?>(StringComparer.Ordinal)
                {
                    ["topic"] = topic,
                    ["payload"] = value.MapGet("payload")?.AsStr(),
                    ["id"] = value.MapGet("id")?.AsStr() ?? string.Empty,
                    ["timestamp"] = value.MapGet("timestamp")?.AsInt() ?? 0L,
                });
            });

            var topicValues = topics.Select(Value.Str).ToArray();
            try
            {
                await client.CallAsync("SUBSCRIBE", topicValues, cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (ThunderException ex)
            {
                throw ToSynapException(ex);
            }

            await foreach (var message in channel.Reader
                .ReadAllAsync(cancellationToken)
                .ConfigureAwait(false))
            {
                yield return message;
            }
        }
        finally
        {
            // Whether the caller stopped enumerating, cancelled, or the stream
            // faulted, the dedicated connection goes with it.
            client.Close();
        }
    }

    /// <summary>
    /// Opens a dedicated push connection driven by <c>KV.WATCH</c> and yields
    /// each raw envelope JSON string — the watch twin of
    /// <see cref="SubscribePushAsync"/>.
    /// </summary>
    /// <remarks>
    /// The push hook is registered before <c>KV.WATCH</c> is sent, so an event
    /// published between the server's acknowledgement and the reader starting
    /// cannot be lost. On the way out — cancellation, abandoned enumeration or
    /// a faulted stream — <c>KV.UNWATCH</c> is issued best-effort before the
    /// dedicated connection closes.
    /// </remarks>
    /// <param name="pattern">Key or wildcard pattern (e.g. <c>user:*</c>).</param>
    /// <param name="mode"><c>value</c> or <c>notify</c>.</param>
    /// <param name="cancellationToken">Token used to stop the stream.</param>
    /// <returns>An async stream of envelope JSON strings.</returns>
    internal async IAsyncEnumerable<string> WatchPushAsync(
        string pattern,
        string mode,
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(pattern);

        var client = await DialAsync(cancellationToken).ConfigureAwait(false);
        var subscriberId = string.Empty;
        try
        {
            var channel = Channel.CreateUnbounded<string>(
                new UnboundedChannelOptions { SingleReader = true, SingleWriter = true });

            client.OnPush(value =>
            {
                // The bridge encodes the envelope as a JSON string.
                var payload = value.MapGet("payload")?.AsStr();
                if (payload is not null)
                {
                    channel.Writer.TryWrite(payload);
                }
            });

            var args = mode == "value"
                ? new[] { Value.Str(pattern) }
                : new[] { Value.Str(pattern), Value.Str(mode) };
            try
            {
                var result = await client.CallAsync("KV.WATCH", args, cancellationToken)
                    .ConfigureAwait(false);
                subscriberId = result.MapGet("subscriber_id")?.AsStr() ?? string.Empty;
            }
            catch (ThunderException ex)
            {
                throw ToSynapException(ex);
            }

            await foreach (var envelope in channel.Reader
                .ReadAllAsync(cancellationToken)
                .ConfigureAwait(false))
            {
                yield return envelope;
            }
        }
        finally
        {
            // Teardown issues KV.UNWATCH so the server drops the routing entry
            // promptly; closing the connection unwinds it anyway, so failures
            // here are swallowed. The caller's token is likely already
            // cancelled, hence CancellationToken.None.
            if (subscriberId.Length > 0)
            {
                try
                {
                    await client.CallAsync(
                        "KV.UNWATCH",
                        new[] { Value.Str(subscriberId) },
                        CancellationToken.None).ConfigureAwait(false);
                }
                catch (ThunderException)
                {
                    // The connection may already be gone.
                }
            }

            client.Close();
        }
    }

    /// <summary>Map a Thunder error onto the SDK's exception type.</summary>
    /// <remarks>
    /// <c>NOAUTH</c> / <c>WRONGPASS</c> / <c>NOPERM</c> arrive as
    /// <see cref="ThunderAuthException"/> because the config selects the RESP3
    /// prefix convention; the server's message travels verbatim either way, so
    /// callers matching on those prefixes keep working.
    /// </remarks>
    private static SynapException ToSynapException(ThunderException ex) => ex switch
    {
        ThunderAuthException or ThunderServerException => SynapException.ServerError(ex.Message),
        _ => SynapException.ServerError($"SynapRPC: {ex.Message}"),
    };

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _client?.Close();
        _client = null;
        _connectLock.Dispose();
    }
}
