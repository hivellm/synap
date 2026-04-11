using Synap.SDK.Exceptions;

namespace Synap.SDK;

/// <summary>
/// Configuration for the Synap client.
///
/// Preferred constructor usage — URL schemes:
/// <code>
///   new SynapConfig("http://localhost:15500")   // HTTP transport
///   new SynapConfig("synap://localhost:15501")  // SynapRPC transport
///   new SynapConfig("resp3://localhost:6379")   // RESP3 transport
/// </code>
/// </summary>
public sealed class SynapConfig
{
    /// <summary>Gets the base URL of the Synap server (used for HTTP transport).</summary>
    [System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1056:URI-like properties should not be strings", Justification = "String is more convenient for users")]
    public string BaseUrl { get; }

    /// <summary>Gets the request timeout in seconds.</summary>
    public int Timeout { get; private set; } = 30;

    /// <summary>Gets the authentication token (API key) for requests.</summary>
    public string? AuthToken { get; private set; }

    /// <summary>Gets the username for Basic Auth.</summary>
    public string? Username { get; private set; }

    /// <summary>Gets the password for Basic Auth.</summary>
    public string? Password { get; private set; }

    /// <summary>Gets the maximum number of retries for failed requests.</summary>
    public int MaxRetries { get; private set; } = 3;

    /// <summary>Gets the transport mode.</summary>
    public TransportMode Transport { get; private set; } = TransportMode.Http;

    /// <summary>Gets the host for SynapRPC transport.</summary>
    public string RpcHost { get; private set; } = "127.0.0.1";

    /// <summary>Gets the port for SynapRPC transport.</summary>
    public int RpcPort { get; private set; } = 15501;

    /// <summary>Gets the host for RESP3 transport.</summary>
    public string Resp3Host { get; private set; } = "127.0.0.1";

    /// <summary>Gets the port for RESP3 transport.</summary>
    public int Resp3Port { get; private set; } = 6379;

    /// <summary>
    /// Initializes a new instance of <see cref="SynapConfig"/> with the default SynapRPC transport.
    ///
    /// Equivalent to <c>new SynapConfig("synap://127.0.0.1:15501")</c>.
    /// SynapRPC (MessagePack over TCP) is the default transport because it is the most
    /// efficient and feature-complete protocol supported by Synap.
    /// </summary>
    public SynapConfig() : this("synap://127.0.0.1:15501")
    {
    }

    /// <summary>
    /// Initializes a new instance of <see cref="SynapConfig"/> from a URL.
    ///
    /// URL schemes:
    /// <list type="bullet">
    ///   <item><c>synap://host:port</c> — SynapRPC transport (DEFAULT, port 15501)</item>
    ///   <item><c>resp3://host:port</c> — RESP3 transport (port 6379)</item>
    ///   <item><c>http://</c> or <c>https://</c> — HTTP transport (port 15500)</item>
    /// </list>
    /// </summary>
    /// <param name="url">The URL with scheme.</param>
    /// <exception cref="SynapException">Thrown when the URL is null or empty.</exception>
    [System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1054:URI-like parameters should not be strings", Justification = "String is more convenient for users")]
    public SynapConfig(string url)
    {
        if (string.IsNullOrWhiteSpace(url))
        {
            throw SynapException.InvalidConfig("Base URL cannot be empty");
        }

        if (url.StartsWith("synap://", StringComparison.OrdinalIgnoreCase))
        {
            var (host, port) = ParseHostPort(url["synap://".Length..], 15_501);
            BaseUrl   = $"http://{host}:15500";
            Transport = TransportMode.SynapRpc;
            RpcHost   = host;
            RpcPort   = port;
        }
        else if (url.StartsWith("resp3://", StringComparison.OrdinalIgnoreCase))
        {
            var (host, port) = ParseHostPort(url["resp3://".Length..], 6_379);
            BaseUrl    = $"http://{host}:15500";
            Transport  = TransportMode.Resp3;
            Resp3Host  = host;
            Resp3Port  = port;
        }
        else
        {
            BaseUrl   = url.TrimEnd('/');
            Transport = TransportMode.Http;
        }
    }

    /// <summary>
    /// Creates a new configuration with the default SynapRPC transport
    /// (<c>synap://127.0.0.1:15501</c>).
    /// </summary>
    /// <returns>A default <see cref="SynapConfig"/> instance.</returns>
    public static SynapConfig Default() => new();

    /// <summary>Creates a new configuration with the specified URL.</summary>
    [System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1054:URI-like parameters should not be strings", Justification = "String is more convenient for users")]
    public static SynapConfig Create(string url) => new(url);

    // ── Immutable builder helpers ──────────────────────────────────────────────

    /// <summary>Returns a copy of this configuration with the specified timeout.</summary>
    public SynapConfig WithTimeout(int timeout)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Timeout = timeout;
        return clone;
    }

    /// <summary>Returns a copy of this configuration with the specified auth token.</summary>
    public SynapConfig WithAuthToken(string token)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.AuthToken = token;
        clone.Username  = null;
        clone.Password  = null;
        return clone;
    }

    /// <summary>Returns a copy of this configuration with Basic Auth credentials.</summary>
    public SynapConfig WithBasicAuth(string username, string password)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Username  = username;
        clone.Password  = password;
        clone.AuthToken = null;
        return clone;
    }

    /// <summary>Returns a copy of this configuration with the specified maximum retries.</summary>
    public SynapConfig WithMaxRetries(int retries)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.MaxRetries = retries;
        return clone;
    }

    /// <summary>
    /// Returns a copy of this configuration using HTTP transport.
    /// </summary>
    /// <remarks>Deprecated: pass an <c>http://</c> URL to the constructor instead.</remarks>
    [Obsolete("Pass an http:// URL to the SynapConfig constructor instead.")]
    public SynapConfig WithHttpTransport()
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Transport = TransportMode.Http;
        return clone;
    }

    /// <summary>
    /// Returns a copy of this configuration using SynapRPC transport.
    /// </summary>
    /// <remarks>Deprecated: pass a <c>synap://</c> URL to the constructor instead.</remarks>
    [Obsolete("Pass a synap://host:port URL to the SynapConfig constructor instead.")]
    public SynapConfig WithSynapRpcTransport()
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Transport = TransportMode.SynapRpc;
        return clone;
    }

    /// <summary>
    /// Returns a copy of this configuration using RESP3 transport.
    /// </summary>
    /// <remarks>Deprecated: pass a <c>resp3://</c> URL to the constructor instead.</remarks>
    [Obsolete("Pass a resp3://host:port URL to the SynapConfig constructor instead.")]
    public SynapConfig WithResp3Transport()
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Transport = TransportMode.Resp3;
        return clone;
    }

    /// <summary>
    /// Returns a copy of this configuration with the specified SynapRPC address.
    /// </summary>
    /// <remarks>Deprecated: pass a <c>synap://host:port</c> URL to the constructor instead.</remarks>
    [Obsolete("Pass a synap://host:port URL to the SynapConfig constructor instead.")]
    public SynapConfig WithRpcAddr(string host, int port)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.RpcHost = host;
        clone.RpcPort = port;
        return clone;
    }

    /// <summary>
    /// Returns a copy of this configuration with the specified RESP3 address.
    /// </summary>
    /// <remarks>Deprecated: pass a <c>resp3://host:port</c> URL to the constructor instead.</remarks>
    [Obsolete("Pass a resp3://host:port URL to the SynapConfig constructor instead.")]
    public SynapConfig WithResp3Addr(string host, int port)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Resp3Host = host;
        clone.Resp3Port = port;
        return clone;
    }

    // ── Internal helpers ───────────────────────────────────────────────────────

    private static (string Host, int Port) ParseHostPort(string hostPort, int defaultPort)
    {
        var lastColon = hostPort.LastIndexOf(':');
        if (lastColon >= 0 && int.TryParse(hostPort[(lastColon + 1)..], out var port) && port is > 0 and <= 65535)
        {
            var host = hostPort[..lastColon];
            return (string.IsNullOrEmpty(host) ? "127.0.0.1" : host, port);
        }

        return (string.IsNullOrEmpty(hostPort) ? "127.0.0.1" : hostPort, defaultPort);
    }
}
