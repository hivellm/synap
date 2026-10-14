using System.Net;
using Moq;
using Moq.Protected;
using Synap.SDK.Modules;

namespace Synap.SDK.Tests.Modules;

/// <summary>
/// GeospatialManager over HTTP, against the command envelope the server
/// really sends. The client unwraps <c>payload</c>, so the manager must read
/// the fields from the unwrapped document.
/// </summary>
public sealed class GeospatialManagerTests : IDisposable
{
    private readonly Mock<HttpMessageHandler> _httpHandlerMock = new();
    private readonly HttpClient _httpClient;
    private readonly SynapClient _client;

    public GeospatialManagerTests()
    {
        _httpClient = new HttpClient(_httpHandlerMock.Object)
        {
            BaseAddress = new Uri("http://localhost:15500"),
        };
        _client = new SynapClient(SynapConfig.Create("http://localhost:15500"), _httpClient);
    }

    private void RespondWithPayload(string payloadJson)
    {
        _httpHandlerMock.Protected()
            .Setup<Task<HttpResponseMessage>>(
                "SendAsync",
                ItExpr.IsAny<HttpRequestMessage>(),
                ItExpr.IsAny<CancellationToken>())
            .ReturnsAsync(() => new HttpResponseMessage
            {
                StatusCode = HttpStatusCode.OK,
                Content = new StringContent(
                    $"{{\"success\":true,\"request_id\":\"r1\",\"payload\":{payloadJson},\"error\":null}}"),
            });
    }

    [Fact]
    public async Task GeoAddAsync_ReturnsTheAddedCountFromTheEnvelope()
    {
        RespondWithPayload("{\"added\":2,\"key\":\"g\"}");
        var added = await _client.Geospatial.GeoAddAsync("g",
        [
            new Location { Lat = 37.7749, Lon = -122.4194, Member = "sf" },
            new Location { Lat = 34.0522, Lon = -118.2437, Member = "la" },
        ]);
        Assert.Equal(2, added);
    }

    [Fact]
    public async Task GeoDistAsync_ReturnsTheDistanceOrNull()
    {
        RespondWithPayload("{\"distance\":559.12,\"key\":\"g\",\"member1\":\"sf\",\"member2\":\"la\",\"unit\":\"km\"}");
        Assert.Equal(559.12, await _client.Geospatial.GeoDistAsync("g", "sf", "la", DistanceUnit.Kilometers));

        RespondWithPayload("{\"distance\":null,\"key\":\"g\",\"member1\":\"sf\",\"member2\":\"nope\",\"unit\":\"km\"}");
        Assert.Null(await _client.Geospatial.GeoDistAsync("g", "sf", "nope", DistanceUnit.Kilometers));
    }

    [Fact]
    public async Task GeoRadiusAsync_ReturnsTheResultsFromTheEnvelope()
    {
        RespondWithPayload("{\"key\":\"g\",\"results\":[{\"coord\":{\"lat\":37.77,\"lon\":-122.41},"
            + "\"distance\":0.5,\"member\":\"sf\"}]}");
        var results = await _client.Geospatial.GeoRadiusAsync(
            "g", 37.7749, -122.4194, 10, DistanceUnit.Kilometers, withDist: true, withCoord: true);
        var sf = Assert.Single(results);
        Assert.Equal("sf", sf.Member);
        Assert.Equal(0.5, sf.Distance);
        Assert.Equal(37.77, sf.Coord!.Lat);
    }

    [Fact]
    public async Task GeoRadiusByMemberAsync_ReturnsTheResultsFromTheEnvelope()
    {
        RespondWithPayload("{\"key\":\"g\",\"results\":[{\"distance\":0.0,\"member\":\"sf\"},"
            + "{\"distance\":559.12,\"member\":\"la\"}]}");
        var results = await _client.Geospatial.GeoRadiusByMemberAsync(
            "g", "sf", 1000, DistanceUnit.Kilometers, withDist: true);
        Assert.Equal(["sf", "la"], results.Select(r => r.Member));
    }

    [Fact]
    public async Task GeoPosAsync_ReturnsTheCoordinatesWithNullForMissingMembers()
    {
        RespondWithPayload("{\"coordinates\":[{\"lat\":37.77,\"lon\":-122.41},null],\"key\":\"g\"}");
        var positions = await _client.Geospatial.GeoPosAsync("g", ["sf", "nope"]);
        Assert.Equal(2, positions.Count);
        Assert.Equal(-122.41, positions[0]!.Lon);
        Assert.Null(positions[1]);
    }

    [Fact]
    public async Task GeoHashAsync_ReturnsTheGeohashesFromTheEnvelope()
    {
        RespondWithPayload("{\"geohashes\":[\"9q8yyk8ytr8\"],\"key\":\"g\"}");
        Assert.Equal(["9q8yyk8ytr8"], await _client.Geospatial.GeoHashAsync("g", ["sf"]));
    }

    [Fact]
    public async Task GeoSearchAsync_ReturnsTheResultsFromTheEnvelope()
    {
        RespondWithPayload("{\"key\":\"g\",\"results\":[{\"member\":\"la\"},{\"member\":\"sf\"}]}");
        var results = await _client.Geospatial.GeoSearchAsync(
            "g", fromLonLat: (-122.4194, 37.7749), byBox: (2000, 2000, DistanceUnit.Kilometers));
        Assert.Equal(["la", "sf"], results.Select(r => r.Member));
    }

    [Fact]
    public async Task StatsAsync_ReadsTheStatisticsFromTheEnvelope()
    {
        RespondWithPayload("{\"geoadd_count\":17,\"geodist_count\":2,\"geohash_count\":2,\"geopos_count\":3,"
            + "\"georadius_count\":8,\"total_keys\":9,\"total_locations\":17}");
        var stats = await _client.Geospatial.StatsAsync();
        Assert.Equal(9, stats.TotalKeys);
        Assert.Equal(17, stats.TotalLocations);
        Assert.Equal(17, stats.GeoaddCount);
        Assert.Equal(8, stats.GeoradiusCount);
    }

    public void Dispose()
    {
        _client.Dispose();
        _httpClient.Dispose();
    }
}
