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
internal static class CommandMapper
{
    /// <summary>
    /// Maps a Synap operation + payload to a native command + args.
    /// Returns null for operations that must fall back to HTTP.
    /// </summary>
    internal static (string Command, object?[] Args)? MapCommand(
        string operation,
        Dictionary<string, object?> payload)
    {
        var key = payload.TryGetValue("key", out var k) ? k?.ToString() ?? string.Empty : string.Empty;

        return operation switch
        {
            // ---- KV ----
            "kv.get" => ("GET", [key]),
            "kv.delete" => ("DEL", [key]),
            "kv.exists" => ("EXISTS", [key]),
            "kv.ttl" => ("TTL", [key]),
            "kv.persist" => ("PERSIST", [key]),
            "kv.type" => ("TYPE", [key]),
            "kv.expire" => payload.TryGetValue("ttl", out var ttl)
                ? ("EXPIRE", new object?[] { key, ttl })
                : null,
            "kv.set" => BuildKvSet(key, payload),
            "kv.incr" => payload.TryGetValue("delta", out var d)
                ? ("INCRBY", new object?[] { key, d })
                : ("INCR", new object?[] { key }),
            "kv.decr" => payload.TryGetValue("delta", out var dd)
                ? ("DECRBY", new object?[] { key, dd })
                : ("DECR", new object?[] { key }),
            "kv.incr_float" => payload.TryGetValue("delta", out var df)
                ? ("INCRBYFLOAT", new object?[] { key, df })
                : null,
            "kv.rename" => payload.TryGetValue("new_key", out var nk)
                ? ("RENAME", new object?[] { key, nk })
                : null,

            // ---- Hash ----
            "hash.set" => BuildHSet(key, payload),
            "hash.get" => payload.TryGetValue("field", out var hf)
                ? ("HGET", new object?[] { key, hf })
                : null,
            "hash.getall" => ("HGETALL", [key]),
            "hash.delete" => payload.TryGetValue("field", out var hdf)
                ? ("HDEL", new object?[] { key, hdf })
                : null,
            "hash.exists" => payload.TryGetValue("field", out var hef)
                ? ("HEXISTS", new object?[] { key, hef })
                : null,
            "hash.len" => ("HLEN", [key]),
            "hash.keys" => ("HKEYS", [key]),
            "hash.values" => ("HVALS", [key]),
            "hash.incr" => payload.TryGetValue("field", out var hif) && payload.TryGetValue("delta", out var hid)
                ? ("HINCRBY", new object?[] { key, hif, hid })
                : null,

            // ---- List ----
            "list.push_left" => payload.TryGetValue("value", out var lpl)
                ? ("LPUSH", new object?[] { key, lpl })
                : null,
            "list.push_right" => payload.TryGetValue("value", out var lpr)
                ? ("RPUSH", new object?[] { key, lpr })
                : null,
            "list.pop_left" => ("LPOP", [key]),
            "list.pop_right" => ("RPOP", [key]),
            "list.len" => ("LLEN", [key]),
            "list.range" => BuildLRange(key, payload),
            "list.index" => payload.TryGetValue("index", out var li)
                ? ("LINDEX", new object?[] { key, li })
                : null,

            // ---- Set ----
            "set.add" => payload.TryGetValue("member", out var sm)
                ? ("SADD", new object?[] { key, sm })
                : null,
            "set.remove" => payload.TryGetValue("member", out var srm)
                ? ("SREM", new object?[] { key, srm })
                : null,
            "set.members" => ("SMEMBERS", [key]),
            "set.ismember" => payload.TryGetValue("member", out var sim)
                ? ("SISMEMBER", new object?[] { key, sim })
                : null,
            "set.card" => ("SCARD", [key]),

            // ---- Sorted Set ----
            "sorted_set.add" => BuildZAdd(key, payload),
            "sorted_set.remove" => payload.TryGetValue("member", out var zrm)
                ? ("ZREM", new object?[] { key, zrm })
                : null,
            "sorted_set.score" => payload.TryGetValue("member", out var zsm)
                ? ("ZSCORE", new object?[] { key, zsm })
                : null,
            "sorted_set.rank" => payload.TryGetValue("member", out var zrk)
                ? ("ZRANK", new object?[] { key, zrk })
                : null,
            "sorted_set.card" => ("ZCARD", [key]),
            "sorted_set.range" => BuildZRange(key, payload),

            // ---- Bitmap ----
            "bitmap.setbit" => payload.TryGetValue("offset", out var bo) && payload.TryGetValue("value", out var bv)
                ? ("SETBIT", new object?[] { key, bo, bv })
                : null,
            "bitmap.getbit" => payload.TryGetValue("offset", out var bgo)
                ? ("GETBIT", new object?[] { key, bgo })
                : null,
            "bitmap.bitcount" => ("BITCOUNT", [key]),
            "bitmap.bitop.and" => BuildBitOp("AND", payload),
            "bitmap.bitop.or" => BuildBitOp("OR", payload),
            "bitmap.bitop.xor" => BuildBitOp("XOR", payload),
            "bitmap.bitop.not" => BuildBitOp("NOT", payload),

            // ---- HyperLogLog ----
            "hyperloglog.add" => payload.TryGetValue("element", out var hle)
                ? ("PFADD", new object?[] { key, hle })
                : null,
            "hyperloglog.count" => ("PFCOUNT", [key]),
            "hyperloglog.pfmerge" => BuildPfMerge(payload),

            // ---- Geospatial ----
            "geospatial.add" => BuildGeoAdd(key, payload),
            "geospatial.dist" => BuildGeoDist(key, payload),
            "geospatial.pos" => payload.TryGetValue("member", out var gpm)
                ? ("GEOPOS", new object?[] { key, gpm })
                : null,
            "geospatial.radius" => BuildGeoRadius(key, payload),

            // Queue, Stream, PubSub, Transaction → HTTP fallback
            _ => null,
        };
    }

    /// <summary>Maps the raw native response back to the Synap response shape.</summary>
    internal static Dictionary<string, object?> MapResponse(string operation, object? raw)
    {
        return operation switch
        {
            "kv.get" => new Dictionary<string, object?> { ["value"] = raw },
            "kv.exists" => new Dictionary<string, object?> { ["exists"] = raw is long l && l > 0 },
            "kv.incr" or "kv.decr" => new Dictionary<string, object?> { ["value"] = raw },
            "kv.incr_float" => new Dictionary<string, object?> { ["value"] = raw },
            "kv.ttl" => new Dictionary<string, object?> { ["ttl"] = raw },
            "kv.type" => new Dictionary<string, object?> { ["type"] = raw },

            "hash.get" => new Dictionary<string, object?> { ["value"] = raw },
            "hash.exists" => new Dictionary<string, object?> { ["exists"] = raw is long hl && hl > 0 },
            "hash.len" => new Dictionary<string, object?> { ["length"] = raw },
            "hash.keys" => new Dictionary<string, object?> { ["keys"] = RawToList(raw) },
            "hash.values" => new Dictionary<string, object?> { ["values"] = RawToList(raw) },
            "hash.getall" => new Dictionary<string, object?> { ["fields"] = FlatArrayToDict(raw) },
            "hash.incr" => new Dictionary<string, object?> { ["value"] = raw },

            "list.pop_left" or "list.pop_right" or "list.index" =>
                new Dictionary<string, object?> { ["value"] = raw },
            "list.len" => new Dictionary<string, object?> { ["length"] = raw },
            "list.range" => new Dictionary<string, object?> { ["items"] = RawToList(raw) },

            "set.ismember" => new Dictionary<string, object?> { ["is_member"] = raw is long sl && sl > 0 },
            "set.card" => new Dictionary<string, object?> { ["count"] = raw },
            "set.members" => new Dictionary<string, object?> { ["members"] = RawToList(raw) },

            "sorted_set.card" => new Dictionary<string, object?> { ["count"] = raw },
            "sorted_set.score" => new Dictionary<string, object?> { ["score"] = raw },
            "sorted_set.rank" => new Dictionary<string, object?> { ["rank"] = raw },
            "sorted_set.range" => new Dictionary<string, object?> { ["members"] = ZRangeToList(raw) },

            "bitmap.getbit" => new Dictionary<string, object?> { ["bit"] = raw },
            "bitmap.bitcount" => new Dictionary<string, object?> { ["count"] = raw },

            "hyperloglog.add" => new Dictionary<string, object?> { ["changed"] = raw is long hll && hll > 0 },
            "hyperloglog.count" => new Dictionary<string, object?> { ["count"] = raw },

            "geospatial.dist" => new Dictionary<string, object?> { ["distance"] = raw },
            "geospatial.pos" => new Dictionary<string, object?> { ["positions"] = RawToList(raw) },
            "geospatial.radius" => new Dictionary<string, object?> { ["members"] = RawToList(raw) },

            // For ops that return no meaningful value (set, delete, etc.)
            _ => new Dictionary<string, object?> { ["success"] = true },
        };
    }

    // -- Helpers --

    private static (string, object?[])? BuildKvSet(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("value", out var value))
        {
            return null;
        }

        if (payload.TryGetValue("ttl", out var ttl))
        {
            return ("SET", [key, value, "EX", ttl]);
        }

        return ("SET", [key, value]);
    }

    private static (string, object?[])? BuildHSet(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("field", out var field) || !payload.TryGetValue("value", out var value))
        {
            return null;
        }

        return ("HSET", [key, field, value]);
    }

    private static (string, object?[])? BuildLRange(string key, Dictionary<string, object?> payload)
    {
        var start = payload.TryGetValue("start", out var s) ? s : (object?)0L;
        var stop = payload.TryGetValue("stop", out var e) ? e : (object?)-1L;
        return ("LRANGE", [key, start, stop]);
    }

    private static (string, object?[])? BuildZAdd(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("member", out var member) || !payload.TryGetValue("score", out var score))
        {
            return null;
        }

        return ("ZADD", [key, score, member]);
    }

    private static (string, object?[])? BuildZRange(string key, Dictionary<string, object?> payload)
    {
        var start = payload.TryGetValue("start", out var s) ? s : (object?)0L;
        var stop = payload.TryGetValue("stop", out var e) ? e : (object?)-1L;
        var withScores = payload.TryGetValue("with_scores", out var ws) && ws is bool b && b;
        if (withScores)
        {
            return ("ZRANGE", [key, start, stop, "WITHSCORES"]);
        }

        return ("ZRANGE", [key, start, stop]);
    }

    private static (string, object?[])? BuildBitOp(string op, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("destination", out var dest))
        {
            return null;
        }

        if (!payload.TryGetValue("keys", out var keysObj))
        {
            return null;
        }

        var keys = keysObj as object?[] ?? Array.Empty<object?>();
        var args = new List<object?> { op, dest };
        args.AddRange(keys);
        return ("BITOP", args.ToArray());
    }

    private static (string, object?[])? BuildPfMerge(Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("destination", out var dest))
        {
            return null;
        }

        if (!payload.TryGetValue("keys", out var keysObj))
        {
            return null;
        }

        var keys = keysObj as object?[] ?? Array.Empty<object?>();
        var args = new List<object?> { dest };
        args.AddRange(keys);
        return ("PFMERGE", args.ToArray());
    }

    private static (string, object?[])? BuildGeoAdd(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("member", out var member))
        {
            return null;
        }

        if (!payload.TryGetValue("longitude", out var lon))
        {
            return null;
        }

        if (!payload.TryGetValue("latitude", out var lat))
        {
            return null;
        }

        return ("GEOADD", [key, lon, lat, member]);
    }

    private static (string, object?[])? BuildGeoDist(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("member1", out var m1))
        {
            return null;
        }

        if (!payload.TryGetValue("member2", out var m2))
        {
            return null;
        }

        var unit = payload.TryGetValue("unit", out var u) ? u?.ToString() ?? "m" : "m";
        return ("GEODIST", [key, m1, m2, unit]);
    }

    private static (string, object?[])? BuildGeoRadius(string key, Dictionary<string, object?> payload)
    {
        if (!payload.TryGetValue("longitude", out var lon))
        {
            return null;
        }

        if (!payload.TryGetValue("latitude", out var lat))
        {
            return null;
        }

        if (!payload.TryGetValue("radius", out var radius))
        {
            return null;
        }

        var unit = payload.TryGetValue("unit", out var u) ? u?.ToString() ?? "m" : "m";
        return ("GEORADIUS", [key, lon, lat, radius, unit]);
    }

    private static List<object?> RawToList(object? raw)
    {
        if (raw is object?[] arr)
        {
            return new List<object?>(arr);
        }

        return new List<object?>();
    }

    private static Dictionary<string, object?> FlatArrayToDict(object? raw)
    {
        var result = new Dictionary<string, object?>();
        if (raw is not object?[] arr)
        {
            return result;
        }

        for (var i = 0; i + 1 < arr.Length; i += 2)
        {
            var k = arr[i]?.ToString();
            if (k is not null)
            {
                result[k] = arr[i + 1];
            }
        }

        return result;
    }

    private static List<object?> ZRangeToList(object? raw)
    {
        if (raw is not object?[] arr)
        {
            return new List<object?>();
        }
        // If WITHSCORES, arr is [member, score, member, score, ...]
        // Return as list of { member, score } dicts
        if (arr.Length > 0 && arr.Length % 2 == 0)
        {
            var hasScores = true;
            for (var i = 1; i < arr.Length; i += 2)
            {
                if (arr[i] is not (double or long or float))
                {
                    hasScores = false;
                    break;
                }
            }

            if (hasScores)
            {
                var list = new List<object?>();
                for (var i = 0; i + 1 < arr.Length; i += 2)
                {
                    list.Add(new Dictionary<string, object?>
                    {
                        ["member"] = arr[i],
                        ["score"] = arr[i + 1],
                    });
                }

                return list;
            }
        }

        return new List<object?>(arr);
    }
}

// ---------------------------------------------------------------------------
// SynapRPC transport — MessagePack over TCP with multiplexing
// ---------------------------------------------------------------------------
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
internal sealed class Resp3Transport : IDisposable
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
    internal async Task<object?> ExecuteAsync(string command, object?[] args, CancellationToken ct = default)
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
