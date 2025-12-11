using System.Text.Json.Serialization;

namespace Synap.SDK.Modules;

/// <summary>
/// Bitfield operation specification.
/// </summary>
public sealed class BitfieldOperation
{
    /// <summary>
    /// Operation type (GET, SET, INCRBY).
    /// </summary>
    [JsonPropertyName("operation")]
    public BitmapManager.BitfieldOperationType Operation { get; set; }

    /// <summary>
    /// Bit offset (0-based).
    /// </summary>
    [JsonPropertyName("offset")]
    public int Offset { get; set; }

    /// <summary>
    /// Bit width (1-64).
    /// </summary>
    [JsonPropertyName("width")]
    public int Width { get; set; }

    /// <summary>
    /// Whether value is signed (default: false).
    /// </summary>
    [JsonPropertyName("signed")]
    public bool? IsSigned { get; set; }

    /// <summary>
    /// Value for SET operation.
    /// </summary>
    [JsonPropertyName("value")]
    public long? Value { get; set; }

    /// <summary>
    /// Increment for INCRBY operation.
    /// </summary>
    [JsonPropertyName("increment")]
    public long? Increment { get; set; }

    /// <summary>
    /// Overflow behavior for INCRBY (default: WRAP).
    /// </summary>
    [JsonPropertyName("overflow")]
    public BitmapManager.BitfieldOverflow? Overflow { get; set; }
}

