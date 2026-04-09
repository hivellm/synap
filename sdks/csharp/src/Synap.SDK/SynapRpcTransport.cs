using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Net.Sockets;
using System.Text;
using Synap.SDK.Exceptions;

namespace Synap.SDK;

internal sealed class SynapRpcTransport : IDisposable
{
    private readonly string _host;
    private readonly int _port;
    private readonly TimeSpan _timeout;
    private TcpClient? _tcp;
    private NetworkStream? _stream;
    private Task? _readerTask;
    private CancellationTokenSource? _cts;
    private readonly ConcurrentDictionary<uint, TaskCompletionSource<object?>> _pending = new();
    private long _nextId;
    private readonly SemaphoreSlim _connectLock = new(1, 1);
    private readonly SemaphoreSlim _writeLock = new(1, 1);
    private bool _disposed;

    internal SynapRpcTransport(string host, int port, int timeoutSeconds)
    {
        _host = host;
        _port = port;
        _timeout = TimeSpan.FromSeconds(timeoutSeconds);
    }

    private async Task EnsureConnectedAsync(CancellationToken ct)
    {
#pragma warning disable CA1508 // analyzer cannot track field writes across async boundaries
        if (_stream is not null)
        {
            return;
        }

        await _connectLock.WaitAsync(ct).ConfigureAwait(false);
        try
        {
            if (_stream is not null)
            {
                return;
            }
#pragma warning restore CA1508

            var tcp = new TcpClient
            {
                ReceiveTimeout = (int)_timeout.TotalMilliseconds,
                SendTimeout = (int)_timeout.TotalMilliseconds,
            };
            await tcp.ConnectAsync(_host, _port, ct).ConfigureAwait(false);
            _tcp = tcp;
            _stream = tcp.GetStream();
            _cts = new CancellationTokenSource();
            _readerTask = RunReaderAsync(_cts.Token);
        }
        finally
        {
            _connectLock.Release();
        }
    }

    private async Task RunReaderAsync(CancellationToken ct)
    {
        try
        {
            while (!ct.IsCancellationRequested && _stream is not null)
            {
                // 4-byte LE length prefix
                var lenBuf = new byte[4];
                await MsgPack.ReadExact(_stream, lenBuf, ct).ConfigureAwait(false);
                var msgLen = BinaryPrimitives.ReadUInt32LittleEndian(lenBuf);

                var msgBuf = new byte[msgLen];
                await MsgPack.ReadExact(_stream, msgBuf, ct).ConfigureAwait(false);

                using var ms = new MemoryStream(msgBuf);
                var decoded = await MsgPack.DecodeAsync(ms, ct).ConfigureAwait(false);

                if (decoded is object?[] arr && arr.Length >= 2)
                {
                    var id = (uint)Convert.ToInt64(arr[0], System.Globalization.CultureInfo.InvariantCulture);
                    if (_pending.TryRemove(id, out var tcs))
                    {
                        if (arr[1] is Dictionary<object, object?> resultMap)
                        {
                            if (resultMap.TryGetValue("Ok", out var okVal))
                            {
                                tcs.SetResult(WireValue.FromWire(okVal));
                            }
                            else if (resultMap.TryGetValue("Err", out var errVal))
                            {
                                tcs.SetException(SynapException.ServerError(errVal?.ToString() ?? "Unknown error"));
                            }
                            else
                            {
                                tcs.SetResult(arr[1]);
                            }
                        }
                        else
                        {
                            tcs.SetResult(arr[1]);
                        }
                    }
                }
            }
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested)
        {
            // Normal shutdown
        }
        catch (IOException ex)
        {
            FailAllPending(ex.Message);
        }
        catch (SocketException ex)
        {
            FailAllPending(ex.Message);
        }
        catch (SynapException ex)
        {
            FailAllPending(ex.Message);
        }
    }

    private void FailAllPending(string message)
    {
        foreach (var tcs in _pending.Values)
        {
            tcs.TrySetException(SynapException.NetworkError(message));
        }

        _pending.Clear();
    }

    /// <summary>Executes a command over SynapRPC and returns the plain (unwrapped) result.</summary>
    internal async Task<object?> ExecuteAsync(string command, object?[] args, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct).ConfigureAwait(false);

        var id = (uint)Interlocked.Increment(ref _nextId);
        var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
        _pending[id] = tcs;

        try
        {
            // Wrap args as WireValues
            var wireArgs = Array.ConvertAll(args, WireValue.ToWire);

            // Build request: [id, COMMAND, [wireArgs...]]
            var request = new object?[] { (long)id, command, wireArgs };
            var msgBytes = MsgPack.Encode(request);

            var lenBuf = new byte[4];
            BinaryPrimitives.WriteUInt32LittleEndian(lenBuf, (uint)msgBytes.Length);

            await _writeLock.WaitAsync(ct).ConfigureAwait(false);
            try
            {
                var stream = _stream!;
                await stream.WriteAsync(lenBuf, ct).ConfigureAwait(false);
                await stream.WriteAsync(msgBytes, ct).ConfigureAwait(false);
                await stream.FlushAsync(ct).ConfigureAwait(false);
            }
            finally
            {
                _writeLock.Release();
            }

            using var linked = CancellationTokenSource.CreateLinkedTokenSource(ct);
            linked.CancelAfter(_timeout);
            return await tcs.Task.WaitAsync(linked.Token).ConfigureAwait(false);
        }
        catch
        {
            _pending.TryRemove(id, out _);
            throw;
        }
    }

    /// <summary>
    /// Opens a dedicated server-push TCP connection, sends a SUBSCRIBE frame,
    /// and yields push messages as an async stream.
    ///
    /// Push frames from the server use id == 0xFFFFFFFF (U32_MAX) as a sentinel.
    /// The stream completes when the cancellation token is cancelled or the
    /// connection is closed by the server.
    /// </summary>
    /// <param name="topics">Topic patterns to subscribe to.</param>
    /// <param name="cancellationToken">Token used to stop the stream.</param>
    /// <returns>An async stream of push-message dictionaries.</returns>
    internal async IAsyncEnumerable<Dictionary<string, object?>> SubscribePushAsync(
        IEnumerable<string> topics,
        [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        const uint PushId = 0xFFFF_FFFF;

        using var tcp = new TcpClient
        {
            ReceiveTimeout = (int)_timeout.TotalMilliseconds,
            SendTimeout    = (int)_timeout.TotalMilliseconds,
        };

        await tcp.ConnectAsync(_host, _port, cancellationToken).ConfigureAwait(false);
        var stream = tcp.GetStream();

        // Build SUBSCRIBE frame: [PushId, "SUBSCRIBE", [topic, ...]]
        var wireTopics = topics.Select(t => WireValue.ToWire(t)).Cast<object?>().ToArray();
        var request    = new object?[] { (long)PushId, "SUBSCRIBE", wireTopics };
        var msgBytes   = MsgPack.Encode(request);
        var lenBuf     = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(lenBuf, (uint)msgBytes.Length);

        await stream.WriteAsync(lenBuf, cancellationToken).ConfigureAwait(false);
        await stream.WriteAsync(msgBytes, cancellationToken).ConfigureAwait(false);
        await stream.FlushAsync(cancellationToken).ConfigureAwait(false);

        // Read SUBSCRIBE ack (id will be PushId)
        var ackLenBuf = new byte[4];
        await MsgPack.ReadExact(stream, ackLenBuf, cancellationToken).ConfigureAwait(false);
        var ackLen = BinaryPrimitives.ReadUInt32LittleEndian(ackLenBuf);
        var ackBuf = new byte[ackLen];
        await MsgPack.ReadExact(stream, ackBuf, cancellationToken).ConfigureAwait(false);

        using var ackMs = new MemoryStream(ackBuf);
        var ack = await MsgPack.DecodeAsync(ackMs, cancellationToken).ConfigureAwait(false);
        if (ack is object?[] ackArr && ackArr.Length >= 2 &&
            ackArr[1] is Dictionary<object, object?> ackMap && ackMap.TryGetValue("Err", out var ackErr))
        {
            throw SynapException.ServerError(ackErr?.ToString() ?? "SUBSCRIBE failed");
        }

        // Read push frames until cancellation
        while (!cancellationToken.IsCancellationRequested)
        {
            var frameLenBuf = new byte[4];
            try
            {
                await MsgPack.ReadExact(stream, frameLenBuf, cancellationToken).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                yield break;
            }
            catch (IOException)
            {
                yield break;
            }

            var frameLen = BinaryPrimitives.ReadUInt32LittleEndian(frameLenBuf);
            var frameBuf = new byte[frameLen];
            await MsgPack.ReadExact(stream, frameBuf, cancellationToken).ConfigureAwait(false);

            using var ms = new MemoryStream(frameBuf);
            var decoded = await MsgPack.DecodeAsync(ms, cancellationToken).ConfigureAwait(false);

            if (decoded is not object?[] pushArr || pushArr.Length < 2)
            {
                continue;
            }

            var frameId = (uint)Convert.ToInt64(pushArr[0], System.Globalization.CultureInfo.InvariantCulture);
            if (frameId != PushId)
            {
                continue; // Not a push frame
            }

            var value = WireValue.FromWire(
                pushArr[1] is Dictionary<object, object?> env && env.TryGetValue("Ok", out var okVal)
                    ? okVal
                    : pushArr[1]);

            if (value is Dictionary<string, object?> msgDict)
            {
                yield return msgDict;
            }
        }

    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _cts?.Cancel();
        _stream?.Dispose();
        _tcp?.Dispose();
        _cts?.Dispose();
        _connectLock.Dispose();
        _writeLock.Dispose();
    }
}

// ---------------------------------------------------------------------------
// RESP3 transport — Redis-compatible text protocol over TCP
// ---------------------------------------------------------------------------
