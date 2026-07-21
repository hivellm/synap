using System.Buffers.Binary;
using System.Net;
using System.Net.Sockets;
using System.Text;
using HiveLLM.Thunder;

namespace Synap.SDK.Tests;

/// <summary>
/// Unit and integration tests for Transport.cs internals:
/// WireValue, CommandMapper, SynapRpcTransport, Resp3Transport.
/// </summary>
public sealed class TransportTests
{
    // =========================================================================
    // WireValue.ToWire — CLR value → Thunder Value
    //
    // The externally-tagged encoding these used to assert is Thunder's now;
    // what stays Synap's is the mapping from the CLR types its command mappers
    // produce, which is what these pin.
    // =========================================================================

    [Fact]
    public void ToWire_Null_ReturnsNullValue()
    {
        Assert.Equal(ValueKind.Null, WireValue.ToWire(null).Kind);
    }

    [Fact]
    public void ToWire_True_ReturnsBool()
    {
        Assert.Equal(true, WireValue.ToWire(true).AsBool());
    }

    [Fact]
    public void ToWire_False_ReturnsBool()
    {
        Assert.Equal(false, WireValue.ToWire(false).AsBool());
    }

    [Fact]
    public void ToWire_Long42_ReturnsInt()
    {
        Assert.Equal(42L, WireValue.ToWire(42L).AsInt());
    }

    [Fact]
    public void ToWire_Int_WidensToInt64()
    {
        Assert.Equal(42L, WireValue.ToWire(42).AsInt());
    }

    [Fact]
    public void ToWire_Double_ReturnsFloat()
    {
        Assert.Equal(3.14, WireValue.ToWire(3.14).AsFloat());
    }

    [Fact]
    public void ToWire_String_ReturnsStr()
    {
        Assert.Equal("hello", WireValue.ToWire("hello").AsStr());
    }

    [Fact]
    public void ToWire_Bytes_ReturnsBytes()
    {
        var bytes = new byte[] { 1, 2, 3 };
        Assert.Equal(bytes, WireValue.ToWire(bytes).AsBytes());
    }

    // =========================================================================
    // WireValue.FromWire — Thunder Value → CLR value
    // =========================================================================

    [Fact]
    public void FromWire_NullValue_ReturnsNull()
    {
        Assert.Null(WireValue.FromWire(Value.Null));
    }

    [Fact]
    public void FromWire_Null_ReturnsNull()
    {
        Assert.Null(WireValue.FromWire(null));
    }

    [Fact]
    public void FromWire_BoolTrue_ReturnsTrue()
    {
        Assert.Equal(true, WireValue.FromWire(Value.Bool(true)));
    }

    [Fact]
    public void FromWire_BoolFalse_ReturnsFalse()
    {
        Assert.Equal(false, WireValue.FromWire(Value.Bool(false)));
    }

    [Fact]
    public void FromWire_Int_ReturnsLong()
    {
        Assert.Equal(42L, WireValue.FromWire(Value.Int(42)));
    }

    [Fact]
    public void FromWire_Float_ReturnsDouble()
    {
        Assert.Equal(3.14, WireValue.FromWire(Value.Float(3.14)));
    }

    [Fact]
    public void FromWire_Str_ReturnsString()
    {
        Assert.Equal("hello", WireValue.FromWire(Value.Str("hello")));
    }

    [Fact]
    public void FromWire_Utf8Bytes_ReturnsString()
    {
        // The SDK surface is string-oriented, so UTF-8 payloads decode to text.
        var bytes = Encoding.UTF8.GetBytes("hello");
        Assert.Equal("hello", WireValue.FromWire(Value.Bytes(bytes)));
    }

    [Fact]
    public void FromWire_NonUtf8Bytes_StaysBinary()
    {
        // 0xFF 0xFE is not valid UTF-8; it must survive as bytes rather than
        // being mangled into replacement characters.
        var bytes = new byte[] { 0xFF, 0xFE };
        Assert.Equal(bytes, WireValue.FromWire(Value.Bytes(bytes)));
    }

    [Fact]
    public void FromWire_Array_ReturnsList()
    {
        var wire = Value.Array(Value.Int(1), Value.Str("two"));
        var result = Assert.IsType<List<object?>>(WireValue.FromWire(wire));
        Assert.Equal(new object?[] { 1L, "two" }, result);
    }

    [Fact]
    public void FromWire_Map_ReturnsDictionary()
    {
        var wire = Value.Map((Value.Str("k"), Value.Int(7)));
        var result = Assert.IsType<Dictionary<string, object?>>(WireValue.FromWire(wire));
        Assert.Equal(7L, result["k"]);
    }

    // =========================================================================
    // Protocol configuration
    // =========================================================================

    [Fact]
    public void SynapConfig_MatchesWhatTheServerDeclares()
    {
        // These values are declared independently in the server's
        // `synap_config()`; a silent change on either side desynchronises them.
        var config = SynapRpcTransport.SynapConfig();
        Assert.Equal("synap", config.Scheme);
        Assert.Equal(15501, config.DefaultPort);
        Assert.Equal(Handshake.AuthCommand, config.Handshake);
        Assert.Equal(HelloStyle.NotUsed, config.HelloStyle);
        Assert.Equal(PushPolicy.Enabled, config.Push);
        Assert.Equal(ErrorConvention.Resp3Prefixes, config.ErrorCodes);
        Assert.Equal(512 * 1024 * 1024, config.MaxFrameBytes);
    }

    // =========================================================================
    // CommandMapper.MapCommand — operation + payload → (command, args)
    // =========================================================================

    [Fact]
    public void MapCommand_KvGet_ReturnsGetCommand()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "foo" };
        var result = CommandMapper.MapCommand("kv.get", payload);
        Assert.True(result.HasValue);
        Assert.Equal("GET", result!.Value.Command);
        Assert.Equal(new object?[] { "foo" }, result.Value.Args);
    }

    [Fact]
    public void MapCommand_KvSet_ReturnsSetCommand()
    {
        var payload = new Dictionary<string, object?>
        {
            ["key"] = "foo",
            ["value"] = new Dictionary<string, object?> { ["Str"] = "bar" },
        };
        var result = CommandMapper.MapCommand("kv.set", payload);
        Assert.True(result.HasValue);
        Assert.Equal("SET", result!.Value.Command);
    }

    [Fact]
    public void MapCommand_WriteWithClientId_WrapsIntoTxQueue()
    {
        var payload = new Dictionary<string, object?>
        {
            ["key"] = "foo",
            ["value"] = "bar",
            ["client_id"] = "tx1",
        };
        var result = CommandMapper.MapCommand("kv.set", payload);
        Assert.True(result.HasValue);
        Assert.Equal("TXQUEUE", result!.Value.Command);
        Assert.Equal("tx1", result.Value.Args[0]);
        Assert.Equal("SET", result.Value.Args[1]);
        Assert.Equal("foo", result.Value.Args[2]);
    }

    [Fact]
    public void MapCommand_UnqueueableWithClientId_ReturnsNull()
    {
        var payload = new Dictionary<string, object?>
        {
            ["key"] = "z",
            ["member"] = "m",
            ["score"] = 1.0,
            ["client_id"] = "tx1",
        };
        var result = CommandMapper.MapCommand("sorted_set.add", payload);
        Assert.False(result.HasValue);
    }

    [Fact]
    public void MapCommand_KvDelete_ReturnsDelCommand()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "foo" };
        var result = CommandMapper.MapCommand("kv.del", payload);
        Assert.True(result.HasValue);
        Assert.Equal("DEL", result!.Value.Command);
        Assert.Equal(new object?[] { "foo" }, result.Value.Args);
    }

    [Fact]
    public void MapCommand_KvExists_ReturnsExistsCommand()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "foo" };
        var result = CommandMapper.MapCommand("kv.exists", payload);
        Assert.True(result.HasValue);
        Assert.Equal("EXISTS", result!.Value.Command);
    }

    [Fact]
    public void MapCommand_KvIncrWithDelta_ReturnsIncrByCommand()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "foo", ["delta"] = 5L };
        var result = CommandMapper.MapCommand("kv.incr", payload);
        Assert.True(result.HasValue);
        Assert.Equal("INCRBY", result!.Value.Command);
        Assert.Equal("foo", result.Value.Args[0]);
        Assert.Equal(5L, result.Value.Args[1]);
    }

    [Fact]
    public void MapCommand_HashGet_ReturnsHGetCommand()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "k", ["field"] = "f" };
        var result = CommandMapper.MapCommand("hash.get", payload);
        Assert.True(result.HasValue);
        Assert.Equal("HGET", result!.Value.Command);
        Assert.Equal("k", result.Value.Args[0]);
        Assert.Equal("f", result.Value.Args[1]);
    }

    [Fact]
    public void MapCommand_ListPushLeft_ReturnsLPushCommand()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "mylist", ["value"] = "item" };
        var result = CommandMapper.MapCommand("list.push_left", payload);
        Assert.True(result.HasValue);
        Assert.Equal("LPUSH", result!.Value.Command);
    }

    [Fact]
    public void MapCommand_SetAdd_ReturnsSAddCommand()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "myset", ["member"] = "m1" };
        var result = CommandMapper.MapCommand("set.add", payload);
        Assert.True(result.HasValue);
        Assert.Equal("SADD", result!.Value.Command);
    }

    [Fact]
    public void MapCommand_QueuePublish_MapsToQPUBLISH()
    {
        // queue.publish is now mapped to QPUBLISH.
        var payload = new Dictionary<string, object?> { ["queue"] = "q", ["payload"] = "msg" };
        var result = CommandMapper.MapCommand("queue.publish", payload);
        Assert.True(result.HasValue);
        Assert.Equal("QPUBLISH", result!.Value.Command);
    }

    [Fact]
    public void MapCommand_StreamPublish_MapsToSPUBLISH()
    {
        // stream.publish is now mapped to SPUBLISH.
        var payload = new Dictionary<string, object?> { ["room"] = "r", ["event"] = "ev", ["data"] = null };
        var result = CommandMapper.MapCommand("stream.publish", payload);
        Assert.True(result.HasValue);
        Assert.Equal("SPUBLISH", result!.Value.Command);
    }

    // =========================================================================
    // CommandMapper.MapResponse — operation + raw → response dict
    // =========================================================================

    [Fact]
    public void MapResponse_KvGet_ReturnsValueKey()
    {
        var result = CommandMapper.MapResponse("kv.get", "bar");
        Assert.Equal("bar", result["value"]);
    }

    [Fact]
    public void MapResponse_KvExists_WhenOne_ReturnsExistsTrue()
    {
        var result = CommandMapper.MapResponse("kv.exists", 1L);
        Assert.Equal(true, result["exists"]);
    }

    [Fact]
    public void MapResponse_KvExists_WhenZero_ReturnsExistsFalse()
    {
        var result = CommandMapper.MapResponse("kv.exists", 0L);
        Assert.Equal(false, result["exists"]);
    }

    [Fact]
    public void MapResponse_KvIncr_ReturnsValueKey()
    {
        var result = CommandMapper.MapResponse("kv.incr", 42L);
        Assert.Equal(42L, result["value"]);
    }

    [Fact]
    public void MapResponse_KvDelete_ReturnsSuccess()
    {
        // kv.del falls to the default case → {"success": true}
        var result = CommandMapper.MapResponse("kv.del", 1L);
        Assert.Equal(true, result["success"]);
    }

    [Fact]
    public void MapResponse_HashGetAll_PairsFlatArrayToDict()
    {
        var raw = new object?[] { "f1", "v1", "f2", "v2" };
        var result = CommandMapper.MapResponse("hash.getall", raw);
        var fields = Assert.IsType<Dictionary<string, object?>>(result["fields"]);
        Assert.Equal("v1", fields["f1"]);
        Assert.Equal("v2", fields["f2"]);
    }

    [Fact]
    public void MapResponse_HashGet_ReturnsValue()
    {
        var result = CommandMapper.MapResponse("hash.get", "fieldval");
        Assert.Equal("fieldval", result["value"]);
    }

    // =========================================================================
    // SynapRpcTransport — end-to-end with a real TcpListener
    // =========================================================================

    [Fact]
    public async Task SynapRpcTransport_ExecuteAsync_SendsRequestAndReceivesResponse()
    {
        using var listener = new TcpListener(IPAddress.Loopback, 0);
        listener.Start();
        var port = ((IPEndPoint)listener.LocalEndpoint).Port;

        // Server task: accept one connection, read one request, send one response
        var serverTask = Task.Run(async () =>
        {
            using var serverClient = await listener.AcceptTcpClientAsync().ConfigureAwait(false);
            using var serverStream = serverClient.GetStream();

            // Read 4-byte LE length prefix
            var lenBuf = new byte[4];
            await MsgPack.ReadExact(serverStream, lenBuf, CancellationToken.None).ConfigureAwait(false);
            var msgLen = BinaryPrimitives.ReadUInt32LittleEndian(lenBuf);

            // Read message body
            var msgBuf = new byte[msgLen];
            await MsgPack.ReadExact(serverStream, msgBuf, CancellationToken.None).ConfigureAwait(false);

            // Decode request to get the id
            using var ms = new MemoryStream(msgBuf);
            var decoded = await MsgPack.DecodeAsync(ms, CancellationToken.None).ConfigureAwait(false);
            var arr = Assert.IsType<object?[]>(decoded);
            var requestId = Convert.ToInt64(arr[0], System.Globalization.CultureInfo.InvariantCulture);

            // Build response: [id, {"Ok": {"Str": "testvalue"}}]
            var okValue = new Dictionary<string, object?> { ["Str"] = "testvalue" };
            var okMap = new Dictionary<string, object?> { ["Ok"] = okValue };
            var response = new object?[] { requestId, okMap };
            var responseBytes = MsgPack.Encode(response);

            var respLenBuf = new byte[4];
            BinaryPrimitives.WriteUInt32LittleEndian(respLenBuf, (uint)responseBytes.Length);
            await serverStream.WriteAsync(respLenBuf, CancellationToken.None).ConfigureAwait(false);
            await serverStream.WriteAsync(responseBytes, CancellationToken.None).ConfigureAwait(false);
            await serverStream.FlushAsync(CancellationToken.None).ConfigureAwait(false);
        });

        using var transport = new SynapRpcTransport("127.0.0.1", port, timeoutSeconds: 10);
        var result = await transport.ExecuteAsync("GET", ["testkey"], CancellationToken.None);

        await serverTask;
        listener.Stop();

        Assert.Equal("testvalue", result);
    }

    [Fact]
    public async Task SynapRpcTransport_RefusesOverCapLengthPrefix()
    {
        // The pre-Thunder transport ran `new byte[msgLen]` with whatever a
        // remote peer's four bytes claimed, so a tiny message could drive an
        // unbounded allocation. Thunder validates against the cap first.
        using var listener = new TcpListener(IPAddress.Loopback, 0);
        listener.Start();
        var port = ((IPEndPoint)listener.LocalEndpoint).Port;

        var serverTask = Task.Run(async () =>
        {
            using var serverClient = await listener.AcceptTcpClientAsync().ConfigureAwait(false);
            using var serverStream = serverClient.GetStream();

            // Drain the request so the client is waiting on a reply.
            var lenBuf = new byte[4];
            await MsgPack.ReadExact(serverStream, lenBuf, CancellationToken.None).ConfigureAwait(false);
            var msgLen = BinaryPrimitives.ReadUInt32LittleEndian(lenBuf);
            await MsgPack.ReadExact(serverStream, new byte[msgLen], CancellationToken.None)
                .ConfigureAwait(false);

            // Answer with a header claiming more than the cap, and no body at
            // all — a client that allocated first would block forever here.
            var overCap = new byte[4];
            BinaryPrimitives.WriteUInt32LittleEndian(overCap, (uint)SynapRpcTransport.MaxFrameBytes + 1);
            await serverStream.WriteAsync(overCap, CancellationToken.None).ConfigureAwait(false);
            await serverStream.FlushAsync(CancellationToken.None).ConfigureAwait(false);

            // Hold the connection open so the refusal is the client's doing.
            await Task.Delay(TimeSpan.FromSeconds(2), CancellationToken.None).ConfigureAwait(false);
        });

        using var transport = new SynapRpcTransport("127.0.0.1", port, timeoutSeconds: 10);

        await Assert.ThrowsAnyAsync<Exception>(
            () => transport.ExecuteAsync("GET", ["k"], CancellationToken.None));

        listener.Stop();
        await serverTask;
    }

    // =========================================================================
    // Resp3Transport — end-to-end with a real TcpListener
    // =========================================================================

    [Fact]
    public async Task Resp3Transport_ExecuteAsync_SendsRequestAndReceivesResponse()
    {
        using var listener = new TcpListener(IPAddress.Loopback, 0);
        listener.Start();
        var port = ((IPEndPoint)listener.LocalEndpoint).Port;

        // Server task: handle HELLO 3, then a GET command
        var serverTask = Task.Run(async () =>
        {
            using var serverClient = await listener.AcceptTcpClientAsync().ConfigureAwait(false);
            using var serverStream = serverClient.GetStream();
            using var reader = new StreamReader(serverStream, Encoding.UTF8, detectEncodingFromByteOrderMarks: false,
                bufferSize: 4096, leaveOpen: true);

            // Read lines until we see the HELLO command, then respond with +OK
            string? line;
            var helloHandled = false;
            while (!helloHandled && (line = await reader.ReadLineAsync().ConfigureAwait(false)) != null)
            {
                if (line.StartsWith("HELLO", StringComparison.OrdinalIgnoreCase)
                    || line == "HELLO")
                {
                    // Flush any remaining HELLO lines
                    var resp = "+OK\r\n"u8.ToArray();
                    await serverStream.WriteAsync(resp, CancellationToken.None).ConfigureAwait(false);
                    await serverStream.FlushAsync(CancellationToken.None).ConfigureAwait(false);
                    helloHandled = true;
                }
            }

            // The client sends HELLO 3 as a RESP array: *2\r\n$5\r\nHELLO\r\n$1\r\n3\r\n
            // ReadLineAsync consumed "*2", then the bulk strings follow.
            // Wait for the GET command array
            // Read until we see "GET" element
            var getHandled = false;
            while (!getHandled && (line = await reader.ReadLineAsync().ConfigureAwait(false)) != null)
            {
                if (line.StartsWith("GET", StringComparison.OrdinalIgnoreCase)
                    || line == "GET")
                {
                    // Drain the key arg line (bulk string header + value)
                    await reader.ReadLineAsync().ConfigureAwait(false); // $7
                    await reader.ReadLineAsync().ConfigureAwait(false); // testkey

                    // Send bulk string response: "$9\r\ntestvalue\r\n"
                    var resp = "$9\r\ntestvalue\r\n"u8.ToArray();
                    await serverStream.WriteAsync(resp, CancellationToken.None).ConfigureAwait(false);
                    await serverStream.FlushAsync(CancellationToken.None).ConfigureAwait(false);
                    getHandled = true;
                }
            }
        });

        using var transport = new Resp3Transport("127.0.0.1", port, timeoutSeconds: 10);
        var result = await transport.ExecuteAsync("GET", ["testkey"], CancellationToken.None);

        await serverTask;
        listener.Stop();

        Assert.Equal("testvalue", result);
    }
}
