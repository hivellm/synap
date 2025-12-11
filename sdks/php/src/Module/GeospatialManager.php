<?php

declare(strict_types=1);

namespace Synap\SDK\Module;

use Synap\SDK\SynapClient;

/**
 * Geospatial operations (Redis-compatible)
 */
class GeospatialManager
{
    public function __construct(
        private SynapClient $client
    ) {
    }

    /**
     * Add geospatial locations (GEOADD)
     *
     * @param array<array{lat: float, lon: float, member: string}> $locations Array of locations
     * @param bool $nx Only add new elements (don't update existing)
     * @param bool $xx Only update existing elements (don't add new)
     * @param bool $ch Return count of changed elements
     * @return int Number of elements added
     */
    public function geoAdd(string $key, array $locations, bool $nx = false, bool $xx = false, bool $ch = false): int
    {
        // Validate coordinates
        foreach ($locations as $loc) {
            $lat = $loc['lat'] ?? null;
            $lon = $loc['lon'] ?? null;
            if ($lat === null || $lon === null) {
                throw new \InvalidArgumentException('Location must have lat and lon');
            }
            if ($lat < -90 || $lat > 90) {
                throw new \InvalidArgumentException("Latitude must be between -90 and 90, got: {$lat}");
            }
            if ($lon < -180 || $lon > 180) {
                throw new \InvalidArgumentException("Longitude must be between -180 and 180, got: {$lon}");
            }
        }

        $response = $this->client->execute('geospatial.geoadd', $key, [
            'locations' => $locations,
            'nx' => $nx,
            'xx' => $xx,
            'ch' => $ch,
        ]);

        $payload = $response['payload'] ?? $response;
        /** @var int|mixed $added */
        $added = $payload['added'] ?? 0;
        return (int) $added;
    }

    /**
     * Calculate distance between two members (GEODIST)
     *
     * @param string $unit Distance unit (m, km, mi, ft)
     * @return float|null Distance in specified unit, or null if either member doesn't exist
     */
    public function geoDist(string $key, string $member1, string $member2, string $unit = 'm'): ?float
    {
        $response = $this->client->execute('geospatial.geodist', $key, [
            'member1' => $member1,
            'member2' => $member2,
            'unit' => $unit,
        ]);

        $payload = $response['payload'] ?? $response;
        $distance = $payload['distance'] ?? null;
        return $distance !== null ? (float) $distance : null;
    }

    /**
     * Query members within radius (GEORADIUS)
     *
     * @param float $centerLat Center latitude
     * @param float $centerLon Center longitude
     * @param float $radius Radius
     * @param string $unit Distance unit (m, km, mi, ft)
     * @param bool $withDist Include distance in results
     * @param bool $withCoord Include coordinates in results
     * @param int|null $count Maximum number of results
     * @param string|null $sort Sort order (ASC, DESC)
     * @return array<int, array{member: string, distance?: float, coord?: array{lat: float, lon: float}}>
     */
    public function geoRadius(
        string $key,
        float $centerLat,
        float $centerLon,
        float $radius,
        string $unit = 'm',
        bool $withDist = false,
        bool $withCoord = false,
        ?int $count = null,
        ?string $sort = null
    ): array {
        if ($centerLat < -90 || $centerLat > 90) {
            throw new \InvalidArgumentException("Latitude must be between -90 and 90, got: {$centerLat}");
        }
        if ($centerLon < -180 || $centerLon > 180) {
            throw new \InvalidArgumentException("Longitude must be between -180 and 180, got: {$centerLon}");
        }

        $data = [
            'center_lat' => $centerLat,
            'center_lon' => $centerLon,
            'radius' => $radius,
            'unit' => $unit,
            'with_dist' => $withDist,
            'with_coord' => $withCoord,
        ];

        if ($count !== null) {
            $data['count'] = $count;
        }
        if ($sort !== null) {
            $data['sort'] = $sort;
        }

        $response = $this->client->execute('geospatial.georadius', $key, $data);
        $payload = $response['payload'] ?? $response;
        return $payload['results'] ?? [];
    }

    /**
     * Query members within radius of given member (GEORADIUSBYMEMBER)
     *
     * @param string $member Center member
     * @param float $radius Radius
     * @param string $unit Distance unit (m, km, mi, ft)
     * @param bool $withDist Include distance in results
     * @param bool $withCoord Include coordinates in results
     * @param int|null $count Maximum number of results
     * @param string|null $sort Sort order (ASC, DESC)
     * @return array<int, array{member: string, distance?: float, coord?: array{lat: float, lon: float}}>
     */
    public function geoRadiusByMember(
        string $key,
        string $member,
        float $radius,
        string $unit = 'm',
        bool $withDist = false,
        bool $withCoord = false,
        ?int $count = null,
        ?string $sort = null
    ): array {
        $data = [
            'member' => $member,
            'radius' => $radius,
            'unit' => $unit,
            'with_dist' => $withDist,
            'with_coord' => $withCoord,
        ];

        if ($count !== null) {
            $data['count'] = $count;
        }
        if ($sort !== null) {
            $data['sort'] = $sort;
        }

        $response = $this->client->execute('geospatial.georadiusbymember', $key, $data);
        $payload = $response['payload'] ?? $response;
        return $payload['results'] ?? [];
    }

    /**
     * Get coordinates of members (GEOPOS)
     *
     * @param array<string> $members Array of member names
     * @return array<int, array{lat: float, lon: float}|null> Array of coordinates (null if member doesn't exist)
     */
    public function geoPos(string $key, array $members): array
    {
        $response = $this->client->execute('geospatial.geopos', $key, ['members' => $members]);
        $payload = $response['payload'] ?? $response;
        return $payload['coordinates'] ?? [];
    }

    /**
     * Get geohash strings for members (GEOHASH)
     *
     * @param array<string> $members Array of member names
     * @return array<int, string|null> Array of geohash strings (null if member doesn't exist)
     */
    public function geoHash(string $key, array $members): array
    {
        $response = $this->client->execute('geospatial.geohash', $key, ['members' => $members]);
        $payload = $response['payload'] ?? $response;
        return $payload['geohashes'] ?? [];
    }

    /**
     * Advanced geospatial search (GEOSEARCH)
     *
     * @param string $key Geospatial key
     * @param string|null $fromMember Center member (mutually exclusive with fromLonLat)
     * @param array{0: float, 1: float}|null $fromLonLat Center coordinates as [lon, lat] (mutually exclusive with fromMember)
     * @param array{0: float, 1: string}|null $byRadius Search by radius as [radius, unit]
     * @param array{0: float, 1: float, 2: string}|null $byBox Search by bounding box as [width, height, unit]
     * @param bool $withDist Include distance in results
     * @param bool $withCoord Include coordinates in results
     * @param bool $withHash Include geohash in results
     * @param int|null $count Maximum number of results
     * @param 'ASC'|'DESC'|null $sort Sort order
     * @return array<int, array{member: string, distance?: float, coord?: array{lat: float, lon: float}}>
     */
    public function geoSearch(
        string $key,
        ?string $fromMember = null,
        ?array $fromLonLat = null,
        ?array $byRadius = null,
        ?array $byBox = null,
        bool $withDist = false,
        bool $withCoord = false,
        bool $withHash = false,
        ?int $count = null,
        ?string $sort = null
    ): array {
        if ($fromMember === null && $fromLonLat === null) {
            throw new \InvalidArgumentException("Either 'fromMember' or 'fromLonLat' must be provided");
        }
        if ($byRadius === null && $byBox === null) {
            throw new \InvalidArgumentException("Either 'byRadius' or 'byBox' must be provided");
        }

        $payload = [
            'with_dist' => $withDist,
            'with_coord' => $withCoord,
            'with_hash' => $withHash,
        ];

        if ($fromMember !== null) {
            $payload['from_member'] = $fromMember;
        }
        if ($fromLonLat !== null) {
            $payload['from_lonlat'] = $fromLonLat;
        }
        if ($byRadius !== null) {
            $payload['by_radius'] = $byRadius;
        }
        if ($byBox !== null) {
            $payload['by_box'] = $byBox;
        }
        if ($count !== null) {
            $payload['count'] = $count;
        }
        if ($sort !== null) {
            $payload['sort'] = $sort;
        }

        $response = $this->client->execute('geospatial.geosearch', $key, $payload);

        $payload = $response['payload'] ?? $response;

        /** @var list<array{member: string, distance?: float, coord?: array{lat: float, lon: float}}> $results */
        $results = $payload['results'] ?? [];
        return $results;
    }

    /**
     * Retrieve geospatial statistics
     *
     * @return array<string, int> Geospatial statistics
     */
    public function stats(): array
    {
        $response = $this->client->execute('geospatial.stats', '', []);
        $payload = $response['payload'] ?? $response;
        return [
            'total_keys' => (int) ($payload['total_keys'] ?? 0),
            'total_locations' => (int) ($payload['total_locations'] ?? 0),
            'geoadd_count' => (int) ($payload['geoadd_count'] ?? 0),
            'geodist_count' => (int) ($payload['geodist_count'] ?? 0),
            'georadius_count' => (int) ($payload['georadius_count'] ?? 0),
            'geopos_count' => (int) ($payload['geopos_count'] ?? 0),
            'geohash_count' => (int) ($payload['geohash_count'] ?? 0),
        ];
    }
}

