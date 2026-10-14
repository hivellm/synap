namespace Synap.SDK.Tests;

/// <summary>
/// A server-to-server test: it talks to a running Synap server, so it runs only
/// when <c>SYNAP_S2S=true</c> and is reported as skipped otherwise.
///
/// A fixed <c>[Fact(Skip = ...)]</c> skips unconditionally, whatever the
/// environment says; setting <see cref="FactAttribute.Skip"/> here, when the
/// attribute is constructed during discovery, is what lets the variable decide.
/// </summary>
public sealed class S2SFactAttribute : FactAttribute
{
    public S2SFactAttribute()
    {
        if (Environment.GetEnvironmentVariable("SYNAP_S2S") != "true")
        {
            Skip = "S2S — set SYNAP_S2S=true to run against a server";
        }
    }
}
