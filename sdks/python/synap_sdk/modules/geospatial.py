"""Geospatial operations for Synap SDK."""

from __future__ import annotations

from typing import TYPE_CHECKING, Literal, TypedDict

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


DistanceUnit = Literal["m", "km", "mi", "ft"]


class Location(TypedDict):
    """Geospatial location coordinate."""

    lat: float
    lon: float
    member: str


class Coordinate(TypedDict):
    """Geographic coordinate."""

    lat: float
    lon: float


class GeoradiusResult(TypedDict, total=False):
    """Result from georadius query."""

    member: str
    distance: float | None
    coord: Coordinate | None


class GeospatialStats:
    """Geospatial statistics."""

    def __init__(self, data: dict[str, int]) -> None:
        """Initialize GeospatialStats from response data."""
        self.total_keys = data.get("total_keys", 0)
        self.total_locations = data.get("total_locations", 0)
        self.geoadd_count = data.get("geoadd_count", 0)
        self.geodist_count = data.get("geodist_count", 0)
        self.georadius_count = data.get("georadius_count", 0)
        self.geopos_count = data.get("geopos_count", 0)
        self.geohash_count = data.get("geohash_count", 0)


class GeospatialManager:
    """Manage Geospatial operations (Redis-compatible).

    Geospatial operations allow location-based queries using latitude/longitude.

    Example:
        >>> async with SynapClient(config) as client:
        ...     await client.geospatial.geoadd("cities", [
        ...         {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"}
        ...     ])
        ...     distance = await client.geospatial.geodist("cities", "SF", "NY", "km")
        ...     results = await client.geospatial.georadius("cities", 37.7749, -122.4194, 100, "km")
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize GeospatialManager.

        Args:
            client: The Synap client instance
        """
        self._client = client

    async def geoadd(
        self,
        key: str,
        locations: list[Location],
        nx: bool = False,
        xx: bool = False,
        ch: bool = False,
    ) -> int:
        """Add geospatial locations (GEOADD).

        Args:
            key: Geospatial key
            locations: Array of locations (lat, lon, member)
            nx: Only add new elements (don't update existing)
            xx: Only update existing elements (don't add new)
            ch: Return count of changed elements

        Returns:
            Number of elements added

        Raises:
            ValueError: If coordinates are out of valid range

        Example:
            >>> added = await geospatial.geoadd("cities", [
            ...     {"lat": 37.7749, "lon": -122.4194, "member": "San Francisco"},
            ...     {"lat": 40.7128, "lon": -74.0060, "member": "New York"}
            ... ])
        """
        # Validate coordinates
        for loc in locations:
            if not -90 <= loc["lat"] <= 90:
                raise ValueError(f"Latitude must be between -90 and 90, got: {loc['lat']}")
            if not -180 <= loc["lon"] <= 180:
                raise ValueError(f"Longitude must be between -180 and 180, got: {loc['lon']}")

        response = await self._client.send_command(
            "geospatial.geoadd",
            {
                "key": key,
                "locations": locations,
                "nx": nx,
                "xx": xx,
                "ch": ch,
            },
        )
        return response.get("added", 0)

    async def geodist(
        self,
        key: str,
        member1: str,
        member2: str,
        unit: DistanceUnit = "m",
    ) -> float | None:
        """Calculate distance between two members (GEODIST).

        Args:
            key: Geospatial key
            member1: First member
            member2: Second member
            unit: Distance unit (default: "m")

        Returns:
            Distance in specified unit, or None if either member doesn't exist

        Example:
            >>> distance = await geospatial.geodist("cities", "San Francisco", "New York", "km")
        """
        response = await self._client.send_command(
            "geospatial.geodist",
            {"key": key, "member1": member1, "member2": member2, "unit": unit},
        )
        return response.get("distance")

    async def georadius(
        self,
        key: str,
        center_lat: float,
        center_lon: float,
        radius: float,
        unit: DistanceUnit = "m",
        *,
        with_dist: bool = False,
        with_coord: bool = False,
        count: int | None = None,
        sort: Literal["ASC", "DESC"] | None = None,
    ) -> list[GeoradiusResult]:
        """Query members within radius (GEORADIUS).

        Args:
            key: Geospatial key
            center_lat: Center latitude
            center_lon: Center longitude
            radius: Radius
            unit: Distance unit (default: "m")
            with_dist: Include distance in results
            with_coord: Include coordinates in results
            count: Maximum number of results
            sort: Sort order ("ASC" or "DESC")

        Returns:
            List of matching members with optional distance and coordinates

        Raises:
            ValueError: If coordinates are out of valid range

        Example:
            >>> results = await geospatial.georadius("cities", 37.7749, -122.4194, 100, "km",
            ...                                      with_dist=True)
        """
        if not -90 <= center_lat <= 90:
            raise ValueError(f"Latitude must be between -90 and 90, got: {center_lat}")
        if not -180 <= center_lon <= 180:
            raise ValueError(f"Longitude must be between -180 and 180, got: {center_lon}")

        payload: dict[str, any] = {
            "key": key,
            "center_lat": center_lat,
            "center_lon": center_lon,
            "radius": radius,
            "unit": unit,
            "with_dist": with_dist,
            "with_coord": with_coord,
        }

        if count is not None:
            payload["count"] = count
        if sort is not None:
            payload["sort"] = sort

        response = await self._client.send_command("geospatial.georadius", payload)
        return response.get("results", [])

    async def georadiusbymember(
        self,
        key: str,
        member: str,
        radius: float,
        unit: DistanceUnit = "m",
        *,
        with_dist: bool = False,
        with_coord: bool = False,
        count: int | None = None,
        sort: Literal["ASC", "DESC"] | None = None,
    ) -> list[GeoradiusResult]:
        """Query members within radius of given member (GEORADIUSBYMEMBER).

        Args:
            key: Geospatial key
            member: Center member
            radius: Radius
            unit: Distance unit (default: "m")
            with_dist: Include distance in results
            with_coord: Include coordinates in results
            count: Maximum number of results
            sort: Sort order ("ASC" or "DESC")

        Returns:
            List of matching members with optional distance and coordinates

        Example:
            >>> results = await geospatial.georadiusbymember("cities", "San Francisco", 50, "km",
            ...                                              with_dist=True)
        """
        payload: dict[str, any] = {
            "key": key,
            "member": member,
            "radius": radius,
            "unit": unit,
            "with_dist": with_dist,
            "with_coord": with_coord,
        }

        if count is not None:
            payload["count"] = count
        if sort is not None:
            payload["sort"] = sort

        response = await self._client.send_command("geospatial.georadiusbymember", payload)
        return response.get("results", [])

    async def geopos(self, key: str, members: list[str]) -> list[Coordinate | None]:
        """Get coordinates of members (GEOPOS).

        Args:
            key: Geospatial key
            members: Array of member names

        Returns:
            List of coordinates (None if member doesn't exist)

        Example:
            >>> coords = await geospatial.geopos("cities", ["San Francisco", "New York"])
        """
        response = await self._client.send_command(
            "geospatial.geopos", {"key": key, "members": members}
        )
        return response.get("coordinates", [])

    async def geohash(self, key: str, members: list[str]) -> list[str | None]:
        """Get geohash strings for members (GEOHASH).

        Args:
            key: Geospatial key
            members: Array of member names

        Returns:
            List of geohash strings (None if member doesn't exist)

        Example:
            >>> geohashes = await geospatial.geohash("cities", ["San Francisco", "New York"])
        """
        response = await self._client.send_command(
            "geospatial.geohash", {"key": key, "members": members}
        )
        return response.get("geohashes", [])

    async def geosearch(
        self,
        key: str,
        *,
        from_member: str | None = None,
        from_lonlat: tuple[float, float] | None = None,
        by_radius: tuple[float, DistanceUnit] | None = None,
        by_box: tuple[float, float, DistanceUnit] | None = None,
        with_dist: bool = False,
        with_coord: bool = False,
        with_hash: bool = False,
        count: int | None = None,
        sort: Literal["ASC", "DESC"] | None = None,
    ) -> list[GeoradiusResult]:
        """Advanced geospatial search (GEOSEARCH).

        Args:
            key: Geospatial key
            from_member: Center member (mutually exclusive with from_lonlat)
            from_lonlat: Center coordinates as (lon, lat) tuple (mutually exclusive with from_member)
            by_radius: Search by radius as (radius, unit) tuple
            by_box: Search by bounding box as (width, height, unit) tuple
            with_dist: Include distance in results
            with_coord: Include coordinates in results
            with_hash: Include geohash in results (not yet implemented)
            count: Maximum number of results
            sort: Sort order ("ASC" or "DESC")

        Returns:
            List of matching members with optional distance and coordinates

        Example:
            >>> results = await geospatial.geosearch(
            ...     "cities",
            ...     from_member="San Francisco",
            ...     by_radius=(50, "km"),
            ...     with_dist=True
            ... )
        """
        if from_member is None and from_lonlat is None:
            raise ValueError("Either 'from_member' or 'from_lonlat' must be provided")
        if by_radius is None and by_box is None:
            raise ValueError("Either 'by_radius' or 'by_box' must be provided")

        payload: dict[str, any] = {
            "key": key,
            "with_dist": with_dist,
            "with_coord": with_coord,
            "with_hash": with_hash,
        }

        if from_member is not None:
            payload["from_member"] = from_member
        if from_lonlat is not None:
            payload["from_lonlat"] = list(from_lonlat)
        if by_radius is not None:
            payload["by_radius"] = [by_radius[0], by_radius[1]]
        if by_box is not None:
            payload["by_box"] = [by_box[0], by_box[1], by_box[2]]
        if count is not None:
            payload["count"] = count
        if sort is not None:
            payload["sort"] = sort

        response = await self._client.send_command("geospatial.geosearch", payload)
        return response.get("results", [])

    async def stats(self) -> GeospatialStats:
        """Retrieve geospatial statistics.

        Returns:
            Geospatial statistics

        Example:
            >>> stats = await geospatial.stats()
            >>> print(stats.total_keys)
        """
        response = await self._client.send_command("geospatial.stats", {})
        return GeospatialStats(response)

