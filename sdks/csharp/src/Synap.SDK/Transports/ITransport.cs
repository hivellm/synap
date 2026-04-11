namespace Synap.SDK.Transports;

/// <summary>
/// Abstraction over the wire transport used by <see cref="SynapClient"/>.
///
/// Three concrete implementations are provided:
/// <list type="bullet">
///   <item><see cref="SynapRpcTransport"/> — MessagePack over TCP (default, port 15501)</item>
///   <item><see cref="Resp3Transport"/> — RESP3 text protocol over TCP (port 6379)</item>
///   <item>HTTP — handled directly in <see cref="SynapClient"/> via <see cref="System.Net.Http.HttpClient"/></item>
/// </list>
/// </summary>
internal interface ITransport : IDisposable
{
    /// <summary>
    /// Executes a native command and returns the decoded result value.
    /// </summary>
    /// <param name="command">The native command name (e.g. "GET", "HSET", "QPUBLISH").</param>
    /// <param name="args">
    /// Positional arguments for the command. Each element must be a primitive
    /// type supported by the transport's serialisation layer.
    /// </param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>
    /// The decoded result value. The concrete type depends on the server
    /// response — callers should use <see cref="CommandMapper.MapResponse"/>
    /// to normalise it into a <see cref="System.Collections.Generic.Dictionary{TKey,TValue}"/>.
    /// </returns>
    public Task<object?> ExecuteAsync(string command, object?[] args, CancellationToken cancellationToken = default);
}
