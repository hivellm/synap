using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Net.Sockets;
using System.Text;
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
// Minimal MessagePack encoder / decoder
// No external dependencies — handles only the types used by SynapRPC.
// ---------------------------------------------------------------------------
internal static class MsgPack
{
    // -----------------------------------------------------------------------
    // Encode
    // -----------------------------------------------------------------------
    internal static byte[] Encode(object? value)
    {
        using var ms = new MemoryStream();
        WriteValue(ms, value);
        return ms.ToArray();
    }

    private static void WriteValue(Stream s, object? v)
    {
        switch (v)
        {
            case null:
                s.WriteByte(0xc0);
                break;
            case bool b:
                s.WriteByte(b ? (byte)0xc3 : (byte)0xc2);
                break;
            case string str:
                WriteStr(s, str);
                break;
            case byte[] bin:
                WriteBin(s, bin);
                break;
            case long l:
                WriteInt64(s, l);
                break;
            case int i:
                WriteInt64(s, i);
                break;
            case uint u:
                WriteUInt64(s, u);
                break;
            case ulong ul:
                WriteUInt64(s, ul);
                break;
            case double d:
                WriteFloat64(s, d);
                break;
            case float f:
                WriteFloat64(s, f);
                break;
            case object?[] arr:
                WriteArray(s, arr);
                break;
            case Dictionary<string, object?> dict:
                WriteMapStrObj(s, dict);
                break;
            default:
                WriteStr(s, v.ToString() ?? string.Empty);
                break;
        }
    }

    private static void WriteStr(Stream s, string str)
    {
        var bytes = Encoding.UTF8.GetBytes(str);
        var len = bytes.Length;
        if (len <= 31)
        {
            s.WriteByte((byte)(0xa0 | len));
        }
        else if (len <= 0xff)
        {
            s.WriteByte(0xd9);
            s.WriteByte((byte)len);
        }
        else if (len <= 0xffff)
        {
            s.WriteByte(0xda);
            WriteBe16(s, (ushort)len);
        }
        else
        {
            s.WriteByte(0xdb);
            WriteBe32(s, (uint)len);
        }

        s.Write(bytes, 0, bytes.Length);
    }

    private static void WriteBin(Stream s, byte[] bytes)
    {
        var len = bytes.Length;
        if (len <= 0xff)
        {
            s.WriteByte(0xc4);
            s.WriteByte((byte)len);
        }
        else if (len <= 0xffff)
        {
            s.WriteByte(0xc5);
            WriteBe16(s, (ushort)len);
        }
        else
        {
            s.WriteByte(0xc6);
            WriteBe32(s, (uint)len);
        }

        s.Write(bytes, 0, bytes.Length);
    }

    private static void WriteInt64(Stream s, long v)
    {
        if (v >= 0)
        {
            WriteUInt64(s, (ulong)v);
        }
        else if (v >= -32)
        {
            s.WriteByte((byte)(0xe0 | (v + 32)));
        }
        else if (v >= -128)
        {
            s.WriteByte(0xd0);
            s.WriteByte((byte)(sbyte)v);
        }
        else if (v >= -32768)
        {
            s.WriteByte(0xd1);
            WriteBe16(s, (ushort)(short)v);
        }
        else if (v >= -2147483648L)
        {
            s.WriteByte(0xd2);
            WriteBe32(s, (uint)(int)v);
        }
        else
        {
            s.WriteByte(0xd3);
            WriteBe64(s, (ulong)v);
        }
    }

    private static void WriteUInt64(Stream s, ulong v)
    {
        if (v <= 127)
        {
            s.WriteByte((byte)v);
        }
        else if (v <= 0xff)
        {
            s.WriteByte(0xcc);
            s.WriteByte((byte)v);
        }
        else if (v <= 0xffff)
        {
            s.WriteByte(0xcd);
            WriteBe16(s, (ushort)v);
        }
        else if (v <= 0xffffffffUL)
        {
            s.WriteByte(0xce);
            WriteBe32(s, (uint)v);
        }
        else
        {
            s.WriteByte(0xcf);
            WriteBe64(s, v);
        }
    }

    private static void WriteFloat64(Stream s, double v)
    {
        s.WriteByte(0xcb);
        Span<byte> buf = stackalloc byte[8];
        BinaryPrimitives.WriteDoubleBigEndian(buf, v);
        s.Write(buf);
    }

    private static void WriteArray(Stream s, object?[] arr)
    {
        var len = arr.Length;
        if (len <= 15)
        {
            s.WriteByte((byte)(0x90 | len));
        }
        else if (len <= 0xffff)
        {
            s.WriteByte(0xdc);
            WriteBe16(s, (ushort)len);
        }
        else
        {
            s.WriteByte(0xdd);
            WriteBe32(s, (uint)len);
        }

        foreach (var item in arr)
        {
            WriteValue(s, item);
        }
    }

    private static void WriteMapStrObj(Stream s, Dictionary<string, object?> dict)
    {
        var len = dict.Count;
        if (len <= 15)
        {
            s.WriteByte((byte)(0x80 | len));
        }
        else if (len <= 0xffff)
        {
            s.WriteByte(0xde);
            WriteBe16(s, (ushort)len);
        }
        else
        {
            s.WriteByte(0xdf);
            WriteBe32(s, (uint)len);
        }

        foreach (var kvp in dict)
        {
            WriteStr(s, kvp.Key);
            WriteValue(s, kvp.Value);
        }
    }

    private static void WriteBe16(Stream s, ushort v)
    {
        Span<byte> b = stackalloc byte[2];
        BinaryPrimitives.WriteUInt16BigEndian(b, v);
        s.Write(b);
    }

    private static void WriteBe32(Stream s, uint v)
    {
        Span<byte> b = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32BigEndian(b, v);
        s.Write(b);
    }

    private static void WriteBe64(Stream s, ulong v)
    {
        Span<byte> b = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64BigEndian(b, v);
        s.Write(b);
    }

    // -----------------------------------------------------------------------
    // Decode (async)
    // -----------------------------------------------------------------------
    internal static async ValueTask<object?> DecodeAsync(Stream s, CancellationToken ct = default)
    {
        var b = await ReadU8(s, ct).ConfigureAwait(false);

        // positive fixint 0x00–0x7f
        if (b <= 0x7f)
        {
            return (long)b;
        }

        // negative fixint 0xe0–0xff
        if (b >= 0xe0)
        {
            return (long)(sbyte)b;
        }

        // fixstr 0xa0–0xbf
        if (b is >= 0xa0 and <= 0xbf)
        {
            return await ReadStr(s, b & 0x1f, ct).ConfigureAwait(false);
        }

        // fixarray 0x90–0x9f
        if (b is >= 0x90 and <= 0x9f)
        {
            return await ReadArray(s, b & 0x0f, ct).ConfigureAwait(false);
        }

        // fixmap 0x80–0x8f
        if (b is >= 0x80 and <= 0x8f)
        {
            return await ReadMap(s, b & 0x0f, ct).ConfigureAwait(false);
        }

        return b switch
        {
            0xc0 => (object?)null,
            0xc2 => false,
            0xc3 => true,
            0xca => (double)await ReadFloat32(s, ct).ConfigureAwait(false),
            0xcb => await ReadFloat64(s, ct).ConfigureAwait(false),
            0xcc => (long)await ReadU8(s, ct).ConfigureAwait(false),
            0xcd => (long)await ReadBe16(s, ct).ConfigureAwait(false),
            0xce => (long)await ReadBe32(s, ct).ConfigureAwait(false),
            0xcf => (long)await ReadBe64(s, ct).ConfigureAwait(false),
            0xd0 => (long)(sbyte)await ReadU8(s, ct).ConfigureAwait(false),
            0xd1 => (long)(short)await ReadBe16(s, ct).ConfigureAwait(false),
            0xd2 => (long)(int)await ReadBe32(s, ct).ConfigureAwait(false),
            0xd3 => await ReadBe64Signed(s, ct).ConfigureAwait(false),
            0xc4 => await ReadBin(s, await ReadU8(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            0xc5 => await ReadBin(s, await ReadBe16(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            0xc6 => await ReadBin(s, (int)await ReadBe32(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            0xd9 => await ReadStr(s, await ReadU8(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            0xda => await ReadStr(s, await ReadBe16(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            0xdb => await ReadStr(s, (int)await ReadBe32(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            0xdc => await ReadArray(s, await ReadBe16(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            0xdd => await ReadArray(s, (int)await ReadBe32(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            0xde => await ReadMap(s, await ReadBe16(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            0xdf => await ReadMap(s, (int)await ReadBe32(s, ct).ConfigureAwait(false), ct).ConfigureAwait(false),
            var x => throw SynapException.InvalidResponse($"Unknown MessagePack prefix 0x{x:x2}"),
        };
    }

    private static async ValueTask<byte> ReadU8(Stream s, CancellationToken ct)
    {
        var buf = new byte[1];
        var n = await s.ReadAsync(buf.AsMemory(0, 1), ct).ConfigureAwait(false);
        if (n == 0)
        {
            throw SynapException.NetworkError("Connection closed");
        }

        return buf[0];
    }

    private static async ValueTask<string> ReadStr(Stream s, int len, CancellationToken ct)
    {
        if (len == 0)
        {
            return string.Empty;
        }

        var buf = new byte[len];
        await ReadExact(s, buf, ct).ConfigureAwait(false);
        return Encoding.UTF8.GetString(buf);
    }

    private static async ValueTask<byte[]> ReadBin(Stream s, int len, CancellationToken ct)
    {
        var buf = new byte[len];
        await ReadExact(s, buf, ct).ConfigureAwait(false);
        return buf;
    }

    private static async ValueTask<object?[]> ReadArray(Stream s, int count, CancellationToken ct)
    {
        var arr = new object?[count];
        for (var i = 0; i < count; i++)
        {
            arr[i] = await DecodeAsync(s, ct).ConfigureAwait(false);
        }

        return arr;
    }

    private static async ValueTask<Dictionary<object, object?>> ReadMap(Stream s, int count, CancellationToken ct)
    {
        var dict = new Dictionary<object, object?>(count);
        for (var i = 0; i < count; i++)
        {
            var key = await DecodeAsync(s, ct).ConfigureAwait(false);
            var val = await DecodeAsync(s, ct).ConfigureAwait(false);
            if (key is not null)
            {
                dict[key] = val;
            }
        }

        return dict;
    }

    private static async ValueTask<ushort> ReadBe16(Stream s, CancellationToken ct)
    {
        var buf = new byte[2];
        await ReadExact(s, buf, ct).ConfigureAwait(false);
        return BinaryPrimitives.ReadUInt16BigEndian(buf);
    }

    private static async ValueTask<uint> ReadBe32(Stream s, CancellationToken ct)
    {
        var buf = new byte[4];
        await ReadExact(s, buf, ct).ConfigureAwait(false);
        return BinaryPrimitives.ReadUInt32BigEndian(buf);
    }

    private static async ValueTask<ulong> ReadBe64(Stream s, CancellationToken ct)
    {
        var buf = new byte[8];
        await ReadExact(s, buf, ct).ConfigureAwait(false);
        return BinaryPrimitives.ReadUInt64BigEndian(buf);
    }

    private static async ValueTask<long> ReadBe64Signed(Stream s, CancellationToken ct)
    {
        var buf = new byte[8];
        await ReadExact(s, buf, ct).ConfigureAwait(false);
        return BinaryPrimitives.ReadInt64BigEndian(buf);
    }

    private static async ValueTask<float> ReadFloat32(Stream s, CancellationToken ct)
    {
        var buf = new byte[4];
        await ReadExact(s, buf, ct).ConfigureAwait(false);
        return BinaryPrimitives.ReadSingleBigEndian(buf);
    }

    private static async ValueTask<double> ReadFloat64(Stream s, CancellationToken ct)
    {
        var buf = new byte[8];
        await ReadExact(s, buf, ct).ConfigureAwait(false);
        return BinaryPrimitives.ReadDoubleBigEndian(buf);
    }

    internal static async Task ReadExact(Stream s, byte[] buf, CancellationToken ct)
    {
        var offset = 0;
        while (offset < buf.Length)
        {
            var n = await s.ReadAsync(buf.AsMemory(offset, buf.Length - offset), ct).ConfigureAwait(false);
            if (n == 0)
            {
                throw SynapException.NetworkError("Connection closed unexpectedly");
            }

            offset += n;
        }
    }
}

// ---------------------------------------------------------------------------
// Wire value helpers for SynapRPC
// ---------------------------------------------------------------------------
internal static class WireValue
{
    /// <summary>Wraps a plain value into externally-tagged WireValue for SynapRPC.</summary>
    internal static object? ToWire(object? v) => v switch
    {
        null => "Null",
        bool b => new Dictionary<string, object?> { ["Bool"] = b },
        string s => new Dictionary<string, object?> { ["Str"] = s },
        byte[] bytes => new Dictionary<string, object?> { ["Bytes"] = bytes },
        long l => new Dictionary<string, object?> { ["Int"] = l },
        int i => new Dictionary<string, object?> { ["Int"] = (long)i },
        double d => new Dictionary<string, object?> { ["Float"] = d },
        float f => new Dictionary<string, object?> { ["Float"] = (double)f },
        _ => new Dictionary<string, object?> { ["Str"] = v.ToString() ?? string.Empty },
    };

    /// <summary>Unwraps an externally-tagged WireValue decoded from MessagePack.</summary>
    internal static object? FromWire(object? wire)
    {
        if (wire is null)
        {
            return null;
        }

        if (wire is string s && s == "Null")
        {
            return null;
        }

        if (wire is Dictionary<object, object?> map)
        {
            if (map.TryGetValue("Str", out var str))
            {
                return str?.ToString();
            }

            if (map.TryGetValue("Int", out var i))
            {
                return Convert.ToInt64(i, System.Globalization.CultureInfo.InvariantCulture);
            }

            if (map.TryGetValue("Float", out var f))
            {
                return Convert.ToDouble(f, System.Globalization.CultureInfo.InvariantCulture);
            }

            if (map.TryGetValue("Bool", out var b))
            {
                return Convert.ToBoolean(b, System.Globalization.CultureInfo.InvariantCulture);
            }

            if (map.TryGetValue("Bytes", out var bytes))
            {
                return bytes;
            }
        }

        return wire;
    }
}

// ---------------------------------------------------------------------------
// Command mapper — translates dot-notation ops to native protocol commands
// ---------------------------------------------------------------------------
