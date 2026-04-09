namespace Synap.SDK.Exceptions;

/// <summary>
/// Raised when a command has no native mapping for the active transport.
///
/// Native transports (SynapRPC, RESP3) can only execute commands that are
/// mapped to wire commands in <see cref="CommandMapper.MapCommand"/>.
/// </summary>
[System.Serializable]
public sealed class UnsupportedCommandException : Exception
{
    /// <summary>Gets the SDK command name that was rejected.</summary>
    public string Command { get; } = string.Empty;

    /// <summary>Gets the transport mode string that rejected the command.</summary>
    public string Transport { get; } = string.Empty;

    /// <summary>Initializes a default instance (required by CA1032).</summary>
    public UnsupportedCommandException() : base() { }

    /// <summary>Initializes with a message (required by CA1032).</summary>
    public UnsupportedCommandException(string message) : base(message) { }

    /// <summary>Initializes with a message and inner exception (required by CA1032).</summary>
    public UnsupportedCommandException(string message, Exception innerException)
        : base(message, innerException) { }

    /// <summary>
    /// Initializes a new instance of <see cref="UnsupportedCommandException"/>.
    /// </summary>
    /// <param name="command">The unsupported command name.</param>
    /// <param name="transport">The active transport mode name.</param>
    public UnsupportedCommandException(string command, string transport)
        : base($"command '{command}' is not supported on transport '{transport}'")
    {
        Command   = command;
        Transport = transport;
    }
}
