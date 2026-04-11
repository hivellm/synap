using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Net.Sockets;
using System.Text;
using Synap.SDK.Exceptions;
using Synap.SDK.Transports;

namespace Synap.SDK;

internal sealed class Resp3Transport : ITransport
{
    private readonly string _host;
    private readonly int _port;
    private readonly TimeSpan _timeout;
    private TcpClient? _tcp;
    private StreamReader? _reader;
    private NetworkStream? _stream;
    private readonly SemaphoreSlim _connectLock = new(1, 1);
    private readonly SemaphoreSlim _requestLock = new(1, 1);
    private bool _disposed;

    internal Resp3Transport(string host, int port, int timeoutSeconds)
    {
        _host = host;
        _port = port;
        _timeout = TimeSpan.FromSeconds(timeoutSeconds);
    }

    private async Task EnsureConnectedAsync(CancellationToken ct)
    {
#pragma warning disable CA1508
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
            _reader = new StreamReader(_stream, Encoding.UTF8, detectEncodingFromByteOrderMarks: false,
                bufferSize: 4096, leaveOpen: true);

            // Send HELLO 3 to enable RESP3 mode
            await SendArrayAsync(["HELLO", "3"], ct).ConfigureAwait(false);
            // Drain the Map response
            await ReadValueAsync(ct).ConfigureAwait(false);
        }
        finally
        {
            _connectLock.Release();
        }
    }

    private async Task SendArrayAsync(string[] parts, CancellationToken ct)
    {
        var sb = new StringBuilder();
        sb.Append(System.Globalization.CultureInfo.InvariantCulture, $"*{parts.Length}\r\n");
        foreach (var part in parts)
        {
            var byteLen = Encoding.UTF8.GetByteCount(part);
            sb.Append(System.Globalization.CultureInfo.InvariantCulture, $"${byteLen}\r\n{part}\r\n");
        }

        var bytes = Encoding.UTF8.GetBytes(sb.ToString());
        await _stream!.WriteAsync(bytes, ct).ConfigureAwait(false);
        await _stream.FlushAsync(ct).ConfigureAwait(false);
    }

    /// <summary>Executes a RESP3 command and returns the decoded result.</summary>
    public async Task<object?> ExecuteAsync(string command, object?[] args, CancellationToken ct = default)
    {
        await EnsureConnectedAsync(ct).ConfigureAwait(false);

        await _requestLock.WaitAsync(ct).ConfigureAwait(false);
        try
        {
            // Build parts: command + string-converted args
            var parts = new string[args.Length + 1];
            parts[0] = command;
            for (var i = 0; i < args.Length; i++)
            {
                parts[i + 1] = ToResp3String(args[i]);
            }

            await SendArrayAsync(parts, ct).ConfigureAwait(false);
            return await ReadValueAsync(ct).ConfigureAwait(false);
        }
        finally
        {
            _requestLock.Release();
        }
    }

    private static string ToResp3String(object? v) => v switch
    {
        null => string.Empty,
        bool b => b ? "1" : "0",
        string s => s,
        double d => d.ToString(System.Globalization.CultureInfo.InvariantCulture),
        float f => f.ToString(System.Globalization.CultureInfo.InvariantCulture),
        long l => l.ToString(System.Globalization.CultureInfo.InvariantCulture),
        int i => i.ToString(System.Globalization.CultureInfo.InvariantCulture),
        _ => v.ToString() ?? string.Empty,
    };

    private async Task<object?> ReadValueAsync(CancellationToken ct)
    {
        var line = await _reader!.ReadLineAsync(ct).ConfigureAwait(false)
            ?? throw SynapException.NetworkError("Connection closed");

        if (line.Length == 0)
        {
            throw SynapException.InvalidResponse("Empty RESP3 line");
        }

        var prefix = line[0];
        var rest = line.Length > 1 ? line[1..] : string.Empty;

        return prefix switch
        {
            '+' => rest,
            '-' => throw SynapException.ServerError(rest),
            ':' => long.Parse(rest, System.Globalization.CultureInfo.InvariantCulture),
            ',' => double.Parse(rest, System.Globalization.CultureInfo.InvariantCulture),
            '#' => rest == "t",
            '_' => (object?)null,
            '$' => await ReadBulkString(rest, ct).ConfigureAwait(false),
            '*' => await ReadRespArray(int.Parse(rest, System.Globalization.CultureInfo.InvariantCulture), ct).ConfigureAwait(false),
            '%' => await ReadRespMap(int.Parse(rest, System.Globalization.CultureInfo.InvariantCulture), ct).ConfigureAwait(false),
            '~' => await ReadRespArray(int.Parse(rest, System.Globalization.CultureInfo.InvariantCulture), ct).ConfigureAwait(false),
            '|' => await SkipAttributesThenRead(int.Parse(rest, System.Globalization.CultureInfo.InvariantCulture), ct).ConfigureAwait(false),
            _ => throw SynapException.InvalidResponse($"Unknown RESP3 prefix '{prefix}'"),
        };
    }

    private async Task<object?> ReadBulkString(string lenStr, CancellationToken ct)
    {
        if (lenStr == "-1")
        {
            return null;
        }

        var len = int.Parse(lenStr, System.Globalization.CultureInfo.InvariantCulture);
        if (len == 0)
        {
            // Read trailing \r\n
            await _reader!.ReadLineAsync(ct).ConfigureAwait(false);
            return string.Empty;
        }

        var buf = new char[len + 2]; // data + CRLF
        var offset = 0;
        while (offset < len + 2)
        {
            var n = await _reader!.ReadAsync(buf.AsMemory(offset, len + 2 - offset), ct).ConfigureAwait(false);
            if (n == 0)
            {
                throw SynapException.NetworkError("Connection closed in bulk string");
            }

            offset += n;
        }

        return new string(buf, 0, len);
    }

    private async Task<object?[]> ReadRespArray(int count, CancellationToken ct)
    {
        if (count <= 0)
        {
            return Array.Empty<object?>();
        }

        var arr = new object?[count];
        for (var i = 0; i < count; i++)
        {
            arr[i] = await ReadValueAsync(ct).ConfigureAwait(false);
        }

        return arr;
    }

    private async Task<Dictionary<object, object?>> ReadRespMap(int count, CancellationToken ct)
    {
        var dict = new Dictionary<object, object?>(count);
        for (var i = 0; i < count; i++)
        {
            var key = await ReadValueAsync(ct).ConfigureAwait(false);
            var val = await ReadValueAsync(ct).ConfigureAwait(false);
            if (key is not null)
            {
                dict[key] = val;
            }
        }

        return dict;
    }

    private async Task<object?> SkipAttributesThenRead(int count, CancellationToken ct)
    {
        for (var i = 0; i < count; i++)
        {
            await ReadValueAsync(ct).ConfigureAwait(false);
            await ReadValueAsync(ct).ConfigureAwait(false);
        }

        return await ReadValueAsync(ct).ConfigureAwait(false);
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _reader?.Dispose();
        _stream?.Dispose();
        _tcp?.Dispose();
        _connectLock.Dispose();
        _requestLock.Dispose();
    }
}
