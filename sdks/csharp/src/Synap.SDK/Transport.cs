using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Net.Sockets;
using System.Text;
using HiveLLM.Thunder;
using Synap.SDK.Exceptions;

namespace Synap.SDK;

/// <summary>Transport mode for the Synap client.</summary>
public enum TransportMode
{
    /// <summary>SynapRPC native protocol over TCP with MessagePack (default, port 15501).</summary>
    SynapRpc,

    /// <summary>RESP3 Redis-compatible protocol over TCP (port 6379).</summary>
    Resp3,

    /// <summary>HTTP/StreamableHTTP protocol (port 15500).</summary>
    Http,
}

// ---------------------------------------------------------------------------
// Wire value helpers for SynapRPC
// ---------------------------------------------------------------------------
internal static class WireValue
{
    /// <summary>
    /// Converts a plain CLR value into a Thunder <see cref="Value"/>.
    /// </summary>
    /// <remarks>
    /// The externally-tagged encoding this used to build by hand is Thunder's
    /// job now; what stays here is Synap's mapping from the CLR types its
    /// command mappers produce.
    /// </remarks>
    internal static Value ToWire(object? v) => v switch
    {
        null => Value.Null,
        bool b => Value.Bool(b),
        string s => Value.Str(s),
        byte[] bytes => Value.Bytes(bytes),
        long l => Value.Int(l),
        int i => Value.Int(i),
        double d => Value.Float(d),
        float f => Value.Float(f),
        _ => Value.Str(v.ToString() ?? string.Empty),
    };

    /// <summary>
    /// Converts a Thunder <see cref="Value"/> back to a plain CLR value.
    /// </summary>
    /// <remarks>
    /// <c>Bytes</c> decode to <see cref="string"/> when they are valid UTF-8 and
    /// stay <c>byte[]</c> otherwise. Thunder handles both the canonical
    /// MessagePack <c>bin</c> form the server emits from 1.1.0 and the legacy
    /// array-of-integers form, so a pre-1.1.0 server still interoperates.
    /// </remarks>
    internal static object? FromWire(Value? wire)
    {
        if (wire is null || wire.Kind == ValueKind.Null)
        {
            return null;
        }

        return wire.Kind switch
        {
            ValueKind.Str => wire.AsStr(),
            ValueKind.Bool => wire.AsBool(),
            ValueKind.Int => wire.AsInt(),
            ValueKind.Float => wire.AsFloat(),
            ValueKind.Bytes => DecodeBytes(wire.AsBytes()),
            ValueKind.Array => wire.AsArray()?.Select(item => FromWire(item)).ToList(),
            ValueKind.Map => wire.AsMap()?.ToDictionary(
                pair => FromWire(pair.Key)?.ToString() ?? string.Empty,
                pair => FromWire(pair.Value),
                StringComparer.Ordinal),
            _ => null,
        };
    }

    /// <summary>UTF-8 bytes surface as a string; anything else stays binary.</summary>
    private static object? DecodeBytes(byte[]? bytes)
    {
        if (bytes is null)
        {
            return null;
        }

        try
        {
            return new UTF8Encoding(encoderShouldEmitUTF8Identifier: false, throwOnInvalidBytes: true)
                .GetString(bytes);
        }
        catch (ArgumentException)
        {
            return bytes;
        }
    }
}

// ---------------------------------------------------------------------------
// Command mapper — translates dot-notation ops to native protocol commands
// ---------------------------------------------------------------------------
