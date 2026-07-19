// Interop cell: C# SDK (HiveLLM.Thunder) against a Thunder-based server.
//
// Driven by scripts/interop/run-matrix.py. Prints one
// `STEP <name> PASS|FAIL <detail>` line per step and exits non-zero if any
// step failed.
//
// Drives SynapRpcTransport directly, the same layer as the Python and
// TypeScript cells, so the matrix measures the wire rather than the JSON
// module surface above it.

using System.Text;
using HiveLLM.Thunder;
using Synap.SDK;

// Not valid UTF-8, so a transport that quietly round-trips through a string
// cannot pass the binary step.
var binary = new byte[] { 0xDE, 0xAD, 0xBE, 0xEF };
const string Topic = "interop.csharp";

var failures = 0;

void Report(string step, bool ok, string detail)
{
    Console.WriteLine($"STEP {step} {(ok ? "PASS" : "FAIL")} {detail}");
    if (!ok)
    {
        failures++;
    }
}

var host = args[0];
var port = int.Parse(args[1]);
var user = args[2];
var pass = args[3];

using var transport = new SynapRpcTransport(
    host, port, timeoutSeconds: 15, credentials: Credentials.UserPass(user, pass));

// 1. Authenticate. Credentials ride the handshake on the first call; the
//    pre-Thunder transport never sent AUTH, so a require_auth server was
//    unreachable.
//
//    EXISTS rather than PING: the server answers PING before authentication,
//    so a PING probe passes just as happily on a connection that never
//    authenticated -- exactly the bug this column exists to catch.
try
{
    var probe = await transport.ExecuteAsync("EXISTS", ["interop:csharp:probe"]);
    Report("auth", true, $"EXISTS -> {probe}");
}
catch (Exception ex)
{
    Report("auth", false, $"{ex.GetType().Name}: {ex.Message}");
    return 1;
}

// 2. SET/GET a binary value -- canonical MessagePack bin, byte-exact back.
try
{
    await transport.ExecuteAsync("SET", ["interop:csharp:bin", binary]);
    var got = await transport.ExecuteAsync("GET", ["interop:csharp:bin"]);
    var bytes = got switch
    {
        byte[] b => b,
        string s => Encoding.UTF8.GetBytes(s),
        _ => Array.Empty<byte>(),
    };
    Report("kv_binary", bytes.SequenceEqual(binary),
        $"{Convert.ToHexString(binary)} -> {Convert.ToHexString(bytes)}");
}
catch (Exception ex)
{
    Report("kv_binary", false, $"{ex.GetType().Name}: {ex.Message}");
}

// 3. SUBSCRIBE then PUBLISH -- the push frame must arrive on the stream.
try
{
    using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(15));
    var received = new List<Dictionary<string, object?>>();

    var reader = Task.Run(async () =>
    {
        await foreach (var push in transport.SubscribePushAsync([Topic], cts.Token))
        {
            received.Add(push);
            await cts.CancelAsync();
            break;
        }
    }, CancellationToken.None);

    await Task.Delay(500, CancellationToken.None);
    await transport.ExecuteAsync("PUBLISH", [Topic, "interop-payload"]);

    try
    {
        await reader;
    }
    catch (OperationCanceledException)
    {
        // Cancelling is how the reader stops after the first frame.
    }

    var ok = received.Count > 0 && (received[0]["topic"] as string) == Topic;
    Report("pubsub", ok, $"received={received.Count} frame(s)");
}
catch (Exception ex)
{
    Report("pubsub", false, $"{ex.GetType().Name}: {ex.Message}");
}

// 4. Error round-trip -- an unknown command must throw, and must not poison
//    the multiplexed connection.
try
{
    var result = await transport.ExecuteAsync("NOSUCHCOMMAND", []);
    Report("error", false, $"expected an exception, got {result}");
}
catch (Exception ex)
{
    var alive = await transport.ExecuteAsync("EXISTS", ["interop:csharp:probe"]) is not null;
    Report("error", alive, $"threw {ex.GetType().Name}; connection alive={alive}");
}

return failures > 0 ? 1 : 0;
