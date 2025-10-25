using Synap.SDK.Exceptions;

namespace Synap.SDK;

/// <summary>
/// Configuration for the Synap client.
/// </summary>
public sealed class SynapConfig
{
    /// <summary>
    /// Gets the base URL of the Synap server.
    /// </summary>
    [System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1056:URI-like properties should not be strings", Justification = "String is more convenient for users")]
    public string BaseUrl { get; }

    /// <summary>
    /// Gets the request timeout in seconds.
    /// </summary>
    public int Timeout { get; private set; } = 30;

    /// <summary>
    /// Gets the authentication token for requests.
    /// </summary>
    public string? AuthToken { get; private set; }

    /// <summary>
    /// Gets the maximum number of retries for failed requests.
    /// </summary>
    public int MaxRetries { get; private set; } = 3;

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

    /// <summary>
    /// Creates a new configuration with the specified base URL.
    /// </summary>
    /// <param name="baseUrl">The base URL of the Synap server.</param>
    /// <returns>A new <see cref="SynapConfig"/> instance.</returns>
    [System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1054:URI-like parameters should not be strings", Justification = "String is more convenient for users")]
    public static SynapConfig Create(string baseUrl) => new(baseUrl);

    /// <summary>
    /// Creates a copy of this configuration with the specified timeout.
    /// </summary>
    /// <param name="timeout">The timeout in seconds.</param>
    /// <returns>A new <see cref="SynapConfig"/> instance with the updated timeout.</returns>
    public SynapConfig WithTimeout(int timeout)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.Timeout = timeout;
        return clone;
    }

    /// <summary>
    /// Creates a copy of this configuration with the specified authentication token.
    /// </summary>
    /// <param name="token">The authentication token.</param>
    /// <returns>A new <see cref="SynapConfig"/> instance with the updated token.</returns>
    public SynapConfig WithAuthToken(string token)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.AuthToken = token;
        return clone;
    }

    /// <summary>
    /// Creates a copy of this configuration with the specified maximum retries.
    /// </summary>
    /// <param name="retries">The maximum number of retries.</param>
    /// <returns>A new <see cref="SynapConfig"/> instance with the updated max retries.</returns>
    public SynapConfig WithMaxRetries(int retries)
    {
        var clone = (SynapConfig)MemberwiseClone();
        clone.MaxRetries = retries;
        return clone;
    }
}

