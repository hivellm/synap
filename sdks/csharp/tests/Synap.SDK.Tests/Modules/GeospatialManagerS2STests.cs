using System.Diagnostics;
using Synap.SDK;
using Synap.SDK.Modules;
using Xunit;

namespace Synap.SDK.Tests.Modules;

/// <summary>
/// S2S integration tests for GeospatialManager.
/// These tests require a running Synap server.
/// </summary>
public class GeospatialManagerS2STests
{
    private readonly SynapClient _client;

    public GeospatialManagerS2STests()
    {
        var url = Environment.GetEnvironmentVariable("SYNAP_URL") ?? "http://localhost:15500";
        _client = new SynapClient(new SynapConfig(url));
    }

    [Fact]
    public async Task GeoAdd_Works()
    {
        var key = $"test:geospatial:{Process.GetCurrentProcess().Id}";
        var locations = new List<Location>
        {
            new Location { Lat = 37.7749, Lon = -122.4194, Member = "San Francisco" },
            new Location { Lat = 40.7128, Lon = -74.0060, Member = "New York" },
        };

        var added = await _client.Geospatial.GeoAddAsync(key, locations);
        Assert.True(added >= 0);
    }

    [Fact]
    public async Task GeoDist_Works()
    {
        var key = $"test:geospatial:dist:{Process.GetCurrentProcess().Id}";
        await _client.Geospatial.GeoAddAsync(key, new List<Location>
        {
            new Location { Lat = 37.7749, Lon = -122.4194, Member = "San Francisco" },
            new Location { Lat = 40.7128, Lon = -74.0060, Member = "New York" },
        });

        var distance = await _client.Geospatial.GeoDistAsync(
            key,
            "San Francisco",
            "New York",
            DistanceUnit.Kilometers
        );

        Assert.NotNull(distance);
        Assert.True(distance > 0);
    }

    [Fact]
    public async Task GeoRadius_Works()
    {
        var key = $"test:geospatial:radius:{Process.GetCurrentProcess().Id}";
        await _client.Geospatial.GeoAddAsync(key, new List<Location>
        {
            new Location { Lat = 37.7749, Lon = -122.4194, Member = "San Francisco" },
            new Location { Lat = 37.8044, Lon = -122.2711, Member = "Oakland" },
        });

        var results = await _client.Geospatial.GeoRadiusAsync(
            key,
            37.7749,
            -122.4194,
            50,
            DistanceUnit.Kilometers,
            withDist: true
        );

        Assert.NotEmpty(results);
    }

    [Fact]
    public async Task GeoPos_Works()
    {
        var key = $"test:geospatial:geopos:{Process.GetCurrentProcess().Id}";
        await _client.Geospatial.GeoAddAsync(key, new List<Location>
        {
            new Location { Lat = 37.7749, Lon = -122.4194, Member = "San Francisco" },
        });

        var coords = await _client.Geospatial.GeoPosAsync(key, new[] { "San Francisco" });
        Assert.Single(coords);
        Assert.NotNull(coords[0]);
        Assert.InRange(coords[0]!.Lat, 37.7, 37.8);
    }

    [Fact]
    public async Task GeoHash_Works()
    {
        var key = $"test:geospatial:geohash:{Process.GetCurrentProcess().Id}";
        await _client.Geospatial.GeoAddAsync(key, new List<Location>
        {
            new Location { Lat = 37.7749, Lon = -122.4194, Member = "San Francisco" },
        });

        var geohashes = await _client.Geospatial.GeoHashAsync(key, new[] { "San Francisco" });
        Assert.Single(geohashes);
        Assert.NotNull(geohashes[0]);
        Assert.Equal(11, geohashes[0]!.Length);
    }

    [Fact]
    public async Task GeoSearch_FromMemberByRadius_Works()
    {
        var key = $"test:geospatial:geosearch:{Process.GetCurrentProcess().Id}";
        await _client.Geospatial.GeoAddAsync(key, new List<Location>
        {
            new Location { Lat = 37.7749, Lon = -122.4194, Member = "San Francisco" },
            new Location { Lat = 37.8044, Lon = -122.2711, Member = "Oakland" },
            new Location { Lat = 40.7128, Lon = -74.0060, Member = "New York" },
        });

        var results = await _client.Geospatial.GeoSearchAsync(
            key,
            fromMember: "San Francisco",
            byRadius: (50.0, DistanceUnit.Kilometers),
            withDist: true
        );

        Assert.NotEmpty(results);
        Assert.Contains(results, r => r.Member == "San Francisco");
    }

    [Fact]
    public async Task GeoSearch_FromLonLatByRadius_Works()
    {
        var key = $"test:geospatial:geosearch:lonlat:{Process.GetCurrentProcess().Id}";
        await _client.Geospatial.GeoAddAsync(key, new List<Location>
        {
            new Location { Lat = 37.7749, Lon = -122.4194, Member = "San Francisco" },
            new Location { Lat = 37.8044, Lon = -122.2711, Member = "Oakland" },
        });

        var results = await _client.Geospatial.GeoSearchAsync(
            key,
            fromLonLat: (-122.4194, 37.7749),
            byRadius: (50.0, DistanceUnit.Kilometers),
            withDist: true,
            withCoord: true
        );

        Assert.NotEmpty(results);
    }

    [Fact]
    public async Task GeoSearch_ByBox_Works()
    {
        var key = $"test:geospatial:geosearch:box:{Process.GetCurrentProcess().Id}";
        await _client.Geospatial.GeoAddAsync(key, new List<Location>
        {
            new Location { Lat = 37.7749, Lon = -122.4194, Member = "San Francisco" },
            new Location { Lat = 37.8044, Lon = -122.2711, Member = "Oakland" },
        });

        var results = await _client.Geospatial.GeoSearchAsync(
            key,
            fromMember: "San Francisco",
            byBox: (100000.0, 100000.0, DistanceUnit.Meters),
            withCoord: true
        );

        Assert.NotEmpty(results);
    }

    [Fact]
    public async Task Stats_Works()
    {
        var stats = await _client.Geospatial.StatsAsync();

        Assert.True(stats.TotalKeys >= 0);
        Assert.True(stats.TotalLocations >= 0);
        Assert.True(stats.GeoaddCount >= 0);
    }
}

