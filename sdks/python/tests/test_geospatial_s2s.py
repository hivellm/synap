"""S2S (Server-to-Server) integration tests for Geospatial operations."""

import os

import pytest

from synap_sdk.client import SynapClient
from synap_sdk.config import SynapConfig


@pytest.fixture
def client():
    """Create a Synap client for testing."""
    config = SynapConfig(
        url=os.getenv("SYNAP_URL", "http://localhost:15500"),
    )
    return SynapClient(config)


class TestGeospatialS2S:
    """S2S integration tests for Geospatial operations."""

    @pytest.mark.asyncio
    async def test_geoadd(self, client):
        """Test GEOADD operation."""
        async with client:
            key = f"test:geospatial:{os.getpid()}"
            locations = [
                {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                {"lat": 40.7128, "lon": -74.0060, "member": "New York"},
            ]
            added = await client.geospatial.geoadd(key, locations)
            assert added >= 0

    @pytest.mark.asyncio
    async def test_geodist(self, client):
        """Test GEODIST operation."""
        async with client:
            key = f"test:geospatial:dist:{os.getpid()}"
            await client.geospatial.geoadd(
                key,
                [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 40.7128, "lon": -74.0060, "member": "New York"},
                ],
            )
            distance = await client.geospatial.geodist(
                key, "San Francisco", "New York", "km"
            )
            assert distance is not None
            assert distance > 0

    @pytest.mark.asyncio
    async def test_georadius(self, client):
        """Test GEORADIUS operation."""
        async with client:
            key = f"test:geospatial:radius:{os.getpid()}"
            await client.geospatial.geoadd(
                key,
                [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 37.8044, "lon": -122.2711, "member": "Oakland"},
                ],
            )
            results = await client.geospatial.georadius(
                key, 37.7749, -122.4194, 50, "km", with_dist=True
            )
            assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_georadiusbymember(self, client):
        """Test GEORADIUSBYMEMBER operation."""
        async with client:
            key = f"test:geospatial:radiusbymember:{os.getpid()}"
            await client.geospatial.geoadd(
                key,
                [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 37.8044, "lon": -122.2711, "member": "Oakland"},
                ],
            )
            results = await client.geospatial.georadiusbymember(
                key, "San Francisco", 50, "km", with_dist=True
            )
            assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_geopos(self, client):
        """Test GEOPOS operation."""
        async with client:
            key = f"test:geospatial:geopos:{os.getpid()}"
            await client.geospatial.geoadd(
                key,
                [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 40.7128, "lon": -74.0060, "member": "New York"},
                ],
            )
            coords = await client.geospatial.geopos(key, ["San Francisco", "New York"])
            assert len(coords) == 2
            assert coords[0] is not None
            assert abs(coords[0]["lat"] - 37.7749) < 0.01

    @pytest.mark.asyncio
    async def test_geohash(self, client):
        """Test GEOHASH operation."""
        async with client:
            key = f"test:geospatial:geohash:{os.getpid()}"
            await client.geospatial.geoadd(
                key,
                [{"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"}],
            )
            geohashes = await client.geospatial.geohash(key, ["San Francisco"])
            assert len(geohashes) == 1
            assert geohashes[0] is not None
            assert len(geohashes[0]) == 11

    @pytest.mark.asyncio
    async def test_geosearch_from_member_by_radius(self, client):
        """Test GEOSEARCH with FROMMEMBER and BYRADIUS."""
        async with client:
            key = f"test:geospatial:geosearch:{os.getpid()}"
            await client.geospatial.geoadd(
                key,
                [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 37.8044, "lon": -122.2711, "member": "Oakland"},
                    {"lat": 40.7128, "lon": -74.0060, "member": "New York"},
                ],
            )
            results = await client.geospatial.geosearch(
                key,
                from_member="San Francisco",
                by_radius=(50, "km"),
                with_dist=True,
            )
            assert len(results) >= 1
            assert any(r["member"] == "San Francisco" for r in results)

    @pytest.mark.asyncio
    async def test_geosearch_from_lonlat_by_radius(self, client):
        """Test GEOSEARCH with FROMLONLAT and BYRADIUS."""
        async with client:
            key = f"test:geospatial:geosearch:lonlat:{os.getpid()}"
            await client.geospatial.geoadd(
                key,
                [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 37.8044, "lon": -122.2711, "member": "Oakland"},
                ],
            )
            results = await client.geospatial.geosearch(
                key,
                from_lonlat=(-122.4194, 37.7749),
                by_radius=(50, "km"),
                with_dist=True,
                with_coord=True,
            )
            assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_geosearch_by_box(self, client):
        """Test GEOSEARCH with BYBOX."""
        async with client:
            key = f"test:geospatial:geosearch:box:{os.getpid()}"
            await client.geospatial.geoadd(
                key,
                [
                    {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
                    {"lat": 37.8044, "lon": -122.2711, "member": "Oakland"},
                ],
            )
            results = await client.geospatial.geosearch(
                key,
                from_member="San Francisco",
                by_box=(100000, 100000, "m"),
                with_coord=True,
            )
            assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_geosearch_count_limit(self, client):
        """Test GEOSEARCH with count limit."""
        async with client:
            key = f"test:geospatial:geosearch:count:{os.getpid()}"
            await client.geospatial.geoadd(
                key,
                [
                    {"lat": 37.7749, "lon": -122.4194, "member": "SF1"},
                    {"lat": 37.7750, "lon": -122.4195, "member": "SF2"},
                    {"lat": 37.7751, "lon": -122.4196, "member": "SF3"},
                ],
            )
            results = await client.geospatial.geosearch(
                key,
                from_lonlat=(-122.4194, 37.7749),
                by_radius=(10, "km"),
                count=2,
            )
            assert len(results) <= 2

    @pytest.mark.asyncio
    async def test_stats(self, client):
        """Test geospatial statistics."""
        async with client:
            stats = await client.geospatial.stats()
            assert hasattr(stats, "total_keys")
            assert hasattr(stats, "total_locations")
            assert hasattr(stats, "geoadd_count")
            assert stats.total_keys >= 0

