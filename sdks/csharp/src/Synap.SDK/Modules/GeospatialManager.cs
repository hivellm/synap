using System.Text.Json;
using System.Text.Json.Serialization;

namespace Synap.SDK.Modules;

/// <summary>
/// Distance unit types.
/// </summary>
public enum DistanceUnit
{
    Meters,
    Kilometers,
    Miles,
    Feet
}

/// <summary>
/// Location with latitude, longitude, and member.
/// </summary>
public class Location
{
    [JsonPropertyName("lat")]
    public double Lat { get; set; }

    [JsonPropertyName("lon")]
    public double Lon { get; set; }

    [JsonPropertyName("member")]
    public string Member { get; set; } = string.Empty;
}

/// <summary>
/// Geographic coordinate.
/// </summary>
public class Coordinate
{
    [JsonPropertyName("lat")]
    public double Lat { get; set; }

    [JsonPropertyName("lon")]
    public double Lon { get; set; }
}

/// <summary>
/// Result from georadius query.
/// </summary>
public class GeoradiusResult
{
    [JsonPropertyName("member")]
    public string Member { get; set; } = string.Empty;

    [JsonPropertyName("distance")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public double? Distance { get; set; }

    [JsonPropertyName("coord")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public Coordinate? Coord { get; set; }
}

/// <summary>
/// Geospatial statistics.
/// </summary>
public class GeospatialStats
{
    [JsonPropertyName("total_keys")]
    public int TotalKeys { get; set; }

    [JsonPropertyName("total_locations")]
    public int TotalLocations { get; set; }

    [JsonPropertyName("geoadd_count")]
    public int GeoaddCount { get; set; }

    [JsonPropertyName("geodist_count")]
    public int GeodistCount { get; set; }

    [JsonPropertyName("georadius_count")]
    public int GeoradiusCount { get; set; }

    [JsonPropertyName("geopos_count")]
    public int GeoposCount { get; set; }

    [JsonPropertyName("geohash_count")]
    public int GeohashCount { get; set; }
}

/// <summary>
/// Geospatial operations (Redis-compatible).
/// </summary>
public sealed class GeospatialManager
{
    private readonly SynapClient _client;

    public GeospatialManager(SynapClient client)
    {
        _client = client;
    }

    private static string DistanceUnitToString(DistanceUnit unit) => unit switch
    {
        DistanceUnit.Meters => "m",
        DistanceUnit.Kilometers => "km",
        DistanceUnit.Miles => "mi",
        DistanceUnit.Feet => "ft",
        _ => "m"
    };

    /// <summary>
    /// Add geospatial locations (GEOADD).
    /// </summary>
    public async Task<int> GeoAddAsync(
        string key,
        IEnumerable<Location> locations,
        bool nx = false,
        bool xx = false,
        bool ch = false,
        CancellationToken cancellationToken = default)
    {
        var locationList = locations.ToList();
        ArgumentNullException.ThrowIfNull(locationList);

        // Validate coordinates
        foreach (var loc in locationList)
        {
            if (loc.Lat < -90 || loc.Lat > 90)
                throw new ArgumentException($"Latitude must be between -90 and 90, got: {loc.Lat}", nameof(locations));
            if (loc.Lon < -180 || loc.Lon > 180)
                throw new ArgumentException($"Longitude must be between -180 and 180, got: {loc.Lon}", nameof(locations));
        }

        var data = new Dictionary<string, object?>
        {
            ["key"] = key,
            ["locations"] = locationList,
            ["nx"] = nx,
            ["xx"] = xx,
            ["ch"] = ch
        };

        using var response = await _client.ExecuteAsync("geospatial.geoadd", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("added", out var added))
        {
            return added.GetInt32();
        }
        return 0;
    }

    /// <summary>
    /// Calculate distance between two members (GEODIST).
    /// </summary>
    public async Task<double?> GeoDistAsync(
        string key,
        string member1,
        string member2,
        DistanceUnit unit = DistanceUnit.Meters,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["key"] = key,
            ["member1"] = member1,
            ["member2"] = member2,
            ["unit"] = DistanceUnitToString(unit)
        };

        using var response = await _client.ExecuteAsync("geospatial.geodist", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("distance", out var distance))
        {
            if (distance.ValueKind == JsonValueKind.Null)
                return null;
            return distance.GetDouble();
        }
        return null;
    }

    /// <summary>
    /// Query members within radius (GEORADIUS).
    /// </summary>
    public async Task<List<GeoradiusResult>> GeoRadiusAsync(
        string key,
        double centerLat,
        double centerLon,
        double radius,
        DistanceUnit unit = DistanceUnit.Meters,
        bool withDist = false,
        bool withCoord = false,
        int? count = null,
        string? sort = null,
        CancellationToken cancellationToken = default)
    {
        if (centerLat < -90 || centerLat > 90)
            throw new ArgumentException($"Latitude must be between -90 and 90, got: {centerLat}", nameof(centerLat));
        if (centerLon < -180 || centerLon > 180)
            throw new ArgumentException($"Longitude must be between -180 and 180, got: {centerLon}", nameof(centerLon));

        var data = new Dictionary<string, object?>
        {
            ["key"] = key,
            ["center_lat"] = centerLat,
            ["center_lon"] = centerLon,
            ["radius"] = radius,
            ["unit"] = DistanceUnitToString(unit),
            ["with_dist"] = withDist,
            ["with_coord"] = withCoord
        };

        if (count.HasValue)
            data["count"] = count.Value;
        if (!string.IsNullOrEmpty(sort))
            data["sort"] = sort;

        using var response = await _client.ExecuteAsync("geospatial.georadius", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("results", out var results))
        {
            return JsonSerializer.Deserialize<List<GeoradiusResult>>(results.GetRawText()) ?? new List<GeoradiusResult>();
        }
        return new List<GeoradiusResult>();
    }

    /// <summary>
    /// Query members within radius of given member (GEORADIUSBYMEMBER).
    /// </summary>
    public async Task<List<GeoradiusResult>> GeoRadiusByMemberAsync(
        string key,
        string member,
        double radius,
        DistanceUnit unit = DistanceUnit.Meters,
        bool withDist = false,
        bool withCoord = false,
        int? count = null,
        string? sort = null,
        CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>
        {
            ["key"] = key,
            ["member"] = member,
            ["radius"] = radius,
            ["unit"] = DistanceUnitToString(unit),
            ["with_dist"] = withDist,
            ["with_coord"] = withCoord
        };

        if (count.HasValue)
            data["count"] = count.Value;
        if (!string.IsNullOrEmpty(sort))
            data["sort"] = sort;

        using var response = await _client.ExecuteAsync("geospatial.georadiusbymember", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("results", out var results))
        {
            return JsonSerializer.Deserialize<List<GeoradiusResult>>(results.GetRawText()) ?? new List<GeoradiusResult>();
        }
        return new List<GeoradiusResult>();
    }

    /// <summary>
    /// Get coordinates of members (GEOPOS).
    /// </summary>
    public async Task<List<Coordinate?>> GeoPosAsync(
        string key,
        IEnumerable<string> members,
        CancellationToken cancellationToken = default)
    {
        var memberList = members.ToList();
        ArgumentNullException.ThrowIfNull(memberList);

        var data = new Dictionary<string, object?>
        {
            ["key"] = key,
            ["members"] = memberList
        };

        using var response = await _client.ExecuteAsync("geospatial.geopos", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("coordinates", out var coordinates))
        {
            return JsonSerializer.Deserialize<List<Coordinate?>>(coordinates.GetRawText()) ?? new List<Coordinate?>();
        }
        return new List<Coordinate?>();
    }

    /// <summary>
    /// Get geohash strings for members (GEOHASH).
    /// </summary>
    public async Task<List<string?>> GeoHashAsync(
        string key,
        IEnumerable<string> members,
        CancellationToken cancellationToken = default)
    {
        var memberList = members.ToList();
        ArgumentNullException.ThrowIfNull(memberList);

        var data = new Dictionary<string, object?>
        {
            ["key"] = key,
            ["members"] = memberList
        };

        using var response = await _client.ExecuteAsync("geospatial.geohash", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload) && payload.TryGetProperty("geohashes", out var geohashes))
        {
            return JsonSerializer.Deserialize<List<string?>>(geohashes.GetRawText()) ?? new List<string?>();
        }
        return new List<string?>();
    }

    /// <summary>
    /// Retrieve geospatial statistics.
    /// </summary>
    public async Task<GeospatialStats> StatsAsync(CancellationToken cancellationToken = default)
    {
        var data = new Dictionary<string, object?>();
        using var response = await _client.ExecuteAsync("geospatial.stats", string.Empty, data, cancellationToken).ConfigureAwait(false);
        if (response.RootElement.TryGetProperty("payload", out var payload))
        {
            return JsonSerializer.Deserialize<GeospatialStats>(payload.GetRawText()) ?? new GeospatialStats();
        }
        return new GeospatialStats();
    }
}

