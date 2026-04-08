using System.Buffers.Binary;
using System.Net;
using System.Net.Sockets;
using System.Text;

namespace Synap.SDK.Tests;

/// <summary>
/// Unit and integration tests for Transport.cs internals:
/// WireValue, CommandMapper, SynapRpcTransport, Resp3Transport.
/// </summary>
public sealed class TransportTests
{
    // =========================================================================
    // WireValue.ToWire — plain → wire-tagged representation
    // =========================================================================

    [Fact]
    public void ToWire_Null_ReturnsNullString()
    {
        var result = WireValue.ToWire(null);
        Assert.Equal("Null", result);
    }

    [Fact]
    public void ToWire_True_ReturnsBoolDict()
    {
        var result = WireValue.ToWire(true);
        var dict = Assert.IsType<Dictionary<string, object?>>(result);
        Assert.Equal(true, dict["Bool"]);
    }

    [Fact]
    public void ToWire_False_ReturnsBoolDict()
    {
        var result = WireValue.ToWire(false);
        var dict = Assert.IsType<Dictionary<string, object?>>(result);
        Assert.Equal(false, dict["Bool"]);
    }

    [Fact]
    public void ToWire_Long42_ReturnsIntDict()
    {
        var result = WireValue.ToWire(42L);
        var dict = Assert.IsType<Dictionary<string, object?>>(result);
        Assert.Equal(42L, dict["Int"]);
    }

    [Fact]
    public void ToWire_Double_ReturnsFloatDict()
    {
        var result = WireValue.ToWire(3.14);
        var dict = Assert.IsType<Dictionary<string, object?>>(result);
        Assert.Equal(3.14, dict["Float"]);
    }

    [Fact]
    public void ToWire_String_ReturnsStrDict()
    {
        var result = WireValue.ToWire("hello");
        var dict = Assert.IsType<Dictionary<string, object?>>(result);
        Assert.Equal("hello", dict["Str"]);
    }

    [Fact]
    public void ToWire_Bytes_ReturnsBytesDict()
    {
        var bytes = new byte[] { 1, 2, 3 };
        var result = WireValue.ToWire(bytes);
        var dict = Assert.IsType<Dictionary<string, object?>>(result);
        Assert.Same(bytes, dict["Bytes"]);
    }

    // =========================================================================
    // WireValue.FromWire — wire-tagged → plain value
    // =========================================================================

    [Fact]
    public void FromWire_NullString_ReturnsNull()
    {
        var result = WireValue.FromWire("Null");
        Assert.Null(result);
    }

    [Fact]
    public void FromWire_Null_ReturnsNull()
    {
        var result = WireValue.FromWire(null);
        Assert.Null(result);
    }

    [Fact]
    public void FromWire_BoolTrueDict_ReturnsTrue()
    {
        var wire = new Dictionary<object, object?> { ["Bool"] = true };
        var result = WireValue.FromWire(wire);
        Assert.Equal(true, result);
    }

    [Fact]
    public void FromWire_BoolFalseDict_ReturnsFalse()
    {
        var wire = new Dictionary<object, object?> { ["Bool"] = false };
        var result = WireValue.FromWire(wire);
        Assert.Equal(false, result);
    }

    [Fact]
    public void FromWire_IntDict_ReturnsLong()
    {
        var wire = new Dictionary<object, object?> { ["Int"] = 42L };
        var result = WireValue.FromWire(wire);
        Assert.Equal(42L, result);
    }

    [Fact]
    public void FromWire_FloatDict_ReturnsDouble()
    {
        var wire = new Dictionary<object, object?> { ["Float"] = 3.14 };
        var result = WireValue.FromWire(wire);
        Assert.Equal(3.14, result);
    }

    [Fact]
    public void FromWire_StrDict_ReturnsString()
    {
        var wire = new Dictionary<object, object?> { ["Str"] = "hello" };
        var result = WireValue.FromWire(wire);
        Assert.Equal("hello", result);
    }

    [Fact]
    public void FromWire_BytesDict_ReturnsByteArray()
    {
        var bytes = new byte[] { 4, 5, 6 };
        var wire = new Dictionary<object, object?> { ["Bytes"] = bytes };
        var result = WireValue.FromWire(wire);
        Assert.Same(bytes, result);
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
    public void MapCommand_KvDelete_ReturnsDelCommand()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "foo" };
        var result = CommandMapper.MapCommand("kv.delete", payload);
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
    public void MapCommand_QueuePublish_ReturnsNull()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "q" };
        var result = CommandMapper.MapCommand("queue.publish", payload);
        Assert.False(result.HasValue);
    }

    [Fact]
    public void MapCommand_StreamPublish_ReturnsNull()
    {
        var payload = new Dictionary<string, object?> { ["key"] = "s" };
        var result = CommandMapper.MapCommand("stream.publish", payload);
        Assert.False(result.HasValue);
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
        // kv.delete falls to the default case → {"success": true}
        var result = CommandMapper.MapResponse("kv.delete", 1L);
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
