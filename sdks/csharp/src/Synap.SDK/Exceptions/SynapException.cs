namespace Synap.SDK.Exceptions;

/// <summary>
/// Base exception for all Synap SDK errors.
/// </summary>
public class SynapException : Exception
{
    /// <summary>
    /// Initializes a new instance of the <see cref="SynapException"/> class.
    /// </summary>
    public SynapException()
    {
    }

    /// <summary>
    /// Initializes a new instance of the <see cref="SynapException"/> class.
    /// </summary>
    /// <param name="message">The error message.</param>
    public SynapException(string message) : base(message)
    {
    }

    /// <summary>
    /// Initializes a new instance of the <see cref="SynapException"/> class.
    /// </summary>
    /// <param name="message">The error message.</param>
    /// <param name="innerException">The inner exception.</param>
    public SynapException(string message, Exception innerException) : base(message, innerException)
    {
    }

    /// <summary>
    /// Creates an HTTP error exception.
    /// </summary>
    /// <param name="message">The error message.</param>
    /// <param name="statusCode">The HTTP status code.</param>
    /// <returns>A new <see cref="SynapException"/> instance.</returns>
    public static SynapException HttpError(string message, int statusCode) =>
        new($"HTTP Error ({statusCode}): {message}");

    /// <summary>
    /// Creates a server error exception.
    /// </summary>
    /// <param name="message">The error message.</param>
    /// <returns>A new <see cref="SynapException"/> instance.</returns>
    public static SynapException ServerError(string message) =>
        new($"Server Error: {message}");

    /// <summary>
    /// Creates a network error exception.
    /// </summary>
    /// <param name="message">The error message.</param>
    /// <returns>A new <see cref="SynapException"/> instance.</returns>
    public static SynapException NetworkError(string message) =>
        new($"Network Error: {message}");

    /// <summary>
    /// Creates an invalid response exception.
    /// </summary>
    /// <param name="message">The error message.</param>
    /// <returns>A new <see cref="SynapException"/> instance.</returns>
    public static SynapException InvalidResponse(string message) =>
        new($"Invalid Response: {message}");

    /// <summary>
    /// Creates an invalid configuration exception.
    /// </summary>
    /// <param name="message">The error message.</param>
    /// <returns>A new <see cref="SynapException"/> instance.</returns>
    public static SynapException InvalidConfig(string message) =>
        new($"Invalid Configuration: {message}");
}

