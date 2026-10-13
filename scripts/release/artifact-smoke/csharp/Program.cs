// Artifact smoke test for the C# SDK.
//
// Run by scripts/release/artifact-smoke/run.py from a scratch copy of this
// project whose only package source for HiveLLM.Synap.SDK is the directory
// holding the freshly packed .nupkg, so the package under test is the one
// about to be published, not the source project.
//
//   argv:   <http-url> <synap-url> <resp3-url>
//   stdout: one `STEP <transport>.<name> PASS|FAIL <detail>` line per step
//   exit:   0 if every step passed

using Synap.SDK;

if (args.Length != 3)
{
    Console.Error.WriteLine("usage: Smoke <http-url> <synap-url> <resp3-url>");
    return 2;
}

var failures = 0;

void Report(string step, bool ok, string detail)
{
    Console.WriteLine($"STEP {step} {(ok ? "PASS" : "FAIL")} {detail}");
    if (!ok)
    {
        failures++;
    }
}

async Task Check(string step, Func<Task<(bool Ok, string Detail)>> run)
{
    try
    {
        var (ok, detail) = await run();
        Report(step, ok, detail);
    }
    catch (Exception ex)
    {
        Report(step, false, $"threw {ex.GetType().Name}: {ex.Message}");
    }
}

SynapClient Connect(string url) => new(SynapConfig.Create(url).WithTimeout(5));

async Task TransportSuite(string name, string url)
{
    using var client = Connect(url);
    var key = $"smoke:cs:{name}:key";
    var counter = $"smoke:cs:{name}:counter";

    await Check($"{name}.set_get", async () =>
    {
        await client.KV.SetAsync(key, $"value-{name}");
        var got = await client.KV.GetAsync<string>(key);
        return (got == $"value-{name}", $"got {got ?? "null"}");
    });
    await Check($"{name}.counter_amounts", async () =>
    {
        await client.KV.SetAsync(counter, "0");
        long[] seen =
        [
            await client.KV.IncrAsync(counter, 10),
            await client.KV.DecrAsync(counter, 4),
            await client.KV.IncrAsync(counter),
        ];
        return (seen.SequenceEqual(new long[] { 10, 6, 7 }), $"incr 10, decr 4, incr -> [{string.Join(",", seen)}]");
    });
    await Check($"{name}.delete", async () =>
    {
        await client.KV.DeleteAsync(key);
        await client.KV.DeleteAsync(counter);
        var got = await client.KV.GetAsync<string>(key);
        return (got is null, $"get after delete -> {got ?? "null"}");
    });
}

await TransportSuite("http", args[0]);
await TransportSuite("synaprpc", args[1]);
await TransportSuite("resp3", args[2]);

using (var http = Connect(args[0]))
using (var rpc = Connect(args[1]))
using (var resp3 = Connect(args[2]))
{
    await Check("cross.http_to_native", async () =>
    {
        const string key = "smoke:cs:cross";
        await http.KV.SetAsync(key, "from-http");
        string?[] seen = [await rpc.KV.GetAsync<string>(key), await resp3.KV.GetAsync<string>(key)];
        await http.KV.DeleteAsync(key);
        return (seen.All(v => v == "from-http"), $"synaprpc, resp3 -> [{string.Join(",", seen)}]");
    });
}

return failures == 0 ? 0 : 1;
