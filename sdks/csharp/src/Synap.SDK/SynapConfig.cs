using Synap.SDK.Exceptions;

namespace Synap.SDK;

/// <summary>
/// Configuration for the Synap client.
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

    /// <summary>Gets the transport mode (default: SynapRpc).</summary>
    public TransportMode Transport { get; private set; } = TransportMode.SynapRpc;

    /// <summary>Gets the host for SynapRPC transport.</summary>
    public string RpcHost { get; private set; } = "127.0.0.1";

    /// <summary>Gets the port for SynapRPC transport.</summary>
    public int RpcPort { get; private set; } = 15501;

    /// <summary>Gets the host for RESP3 transport.</summary>
    public string Resp3Host { get; private set; } = "127.0.0.1";

    /// <summary>Gets the port for RESP3 transport.</summary>
    public int Resp3Port { get; private set; } = 6379;

    /// <summary>
    /// Initializes a new instance of the <see cref="SynapConfig"/> class.
    /// </summary>
    /// <param name="baseUrl">The base URL of the Synap server.</param>
    /// <exception cref="SynapException">Thrown when the base URL is null or empty.</exception>
    [System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1054:URI-like parameters should not be strings", Justification = "String is more convenient for users")]
    public SynapConfig(string baseUrl)
    {
        if (string.IsNullOrWhiteSpace(baseUrl))
        {
            throw SynapException.InvalidConfig("Base URL cannot be empty");
        }

        BaseUrl = baseUrl.TrimEnd('/');
    }

    /// <summary>Creates a new configuration with the specified base URL.</summary>
    [System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1054:URI-like parameters should not be strings", Justification = "String is more convenient for users")]
    public static SynapConfig Create(string baseUrl) => new(baseUrl);

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
        clone.Username = null;
        clone.Password = null;
        return clone;
    }

    /// <summary>Returns a copy of this configuration with Basic Auth credentials.</summary>
    public SynapConfig WithBasicAuth(string username, string password)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Username = username;
        clone.Password = password;
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

    /// <summary>Returns a copy of this configuration using HTTP transport.</summary>
    public SynapConfig WithHttpTransport()
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Transport = TransportMode.Http;
        return clone;
    }

    /// <summary>Returns a copy of this configuration using SynapRPC transport (default).</summary>
    public SynapConfig WithSynapRpcTransport()
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Transport = TransportMode.SynapRpc;
        return clone;
    }

    /// <summary>Returns a copy of this configuration using RESP3 transport.</summary>
    public SynapConfig WithResp3Transport()
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Transport = TransportMode.Resp3;
        return clone;
    }

    /// <summary>Returns a copy of this configuration with the specified SynapRPC address.</summary>
    public SynapConfig WithRpcAddr(string host, int port)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.RpcHost = host;
        clone.RpcPort = port;
        return clone;
    }

    /// <summary>Returns a copy of this configuration with the specified RESP3 address.</summary>
    public SynapConfig WithResp3Addr(string host, int port)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Resp3Host = host;
        clone.Resp3Port = port;
        return clone;
    }
}
