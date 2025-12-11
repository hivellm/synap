<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Module\GeospatialManager;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

/**
 * @group s2s
 * @requires extension curl
 */
final class GeospatialManagerS2STest extends TestCase
{
    private GeospatialManager $geospatial;
    private string $testKey;

    protected function setUp(): void
    {
        $url = getenv('SYNAP_URL') ?: 'http://localhost:15500';
        $config = new SynapConfig($url);
        $client = new SynapClient($config);
        $this->geospatial = new GeospatialManager($client);
        $this->testKey = 'test:geospatial:' . getmypid();
    }

    public function testGeoAdd(): void
    {
        $locations = [
            ['lat' => 37.7749, 'lon' => -122.4194, 'member' => 'San Francisco'],
            ['lat' => 40.7128, 'lon' => -74.0060, 'member' => 'New York'],
        ];

        $added = $this->geospatial->geoAdd($this->testKey, $locations);
        $this->assertGreaterThanOrEqual(0, $added);
    }

    public function testGeoDist(): void
    {
        $key = $this->testKey . ':dist';
        $this->geospatial->geoAdd($key, [
            ['lat' => 37.7749, 'lon' => -122.4194, 'member' => 'San Francisco'],
            ['lat' => 40.7128, 'lon' => -74.0060, 'member' => 'New York'],
        ]);

        $distance = $this->geospatial->geoDist($key, 'San Francisco', 'New York', 'km');
        $this->assertNotNull($distance);
        $this->assertGreaterThan(0, $distance);
    }

    public function testGeoRadius(): void
    {
        $key = $this->testKey . ':radius';
        $this->geospatial->geoAdd($key, [
            ['lat' => 37.7749, 'lon' => -122.4194, 'member' => 'San Francisco'],
            ['lat' => 37.8044, 'lon' => -122.2711, 'member' => 'Oakland'],
        ]);

        $results = $this->geospatial->geoRadius(
            $key,
            37.7749,
            -122.4194,
            50,
            'km',
            withDist: true
        );

        $this->assertNotEmpty($results);
    }

    public function testGeoPos(): void
    {
        $key = $this->testKey . ':geopos';
        $this->geospatial->geoAdd($key, [
            ['lat' => 37.7749, 'lon' => -122.4194, 'member' => 'San Francisco'],
        ]);

        $coords = $this->geospatial->geoPos($key, ['San Francisco']);
        $this->assertCount(1, $coords);
        $this->assertNotNull($coords[0]);
        $this->assertArrayHasKey('lat', $coords[0]);
        $this->assertArrayHasKey('lon', $coords[0]);
    }

    public function testGeoHash(): void
    {
        $key = $this->testKey . ':geohash';
        $this->geospatial->geoAdd($key, [
            ['lat' => 37.7749, 'lon' => -122.4194, 'member' => 'San Francisco'],
        ]);

        $geohashes = $this->geospatial->geoHash($key, ['San Francisco']);
        $this->assertCount(1, $geohashes);
        $this->assertNotNull($geohashes[0]);
        $this->assertEquals(11, strlen($geohashes[0]));
    }

    public function testGeoSearchFromMemberByRadius(): void
    {
        $key = $this->testKey . ':geosearch';
        $this->geospatial->geoAdd($key, [
            ['lat' => 37.7749, 'lon' => -122.4194, 'member' => 'San Francisco'],
            ['lat' => 37.8044, 'lon' => -122.2711, 'member' => 'Oakland'],
            ['lat' => 40.7128, 'lon' => -74.0060, 'member' => 'New York'],
        ]);

        $results = $this->geospatial->geoSearch(
            $key,
            fromMember: 'San Francisco',
            byRadius: [50.0, 'km'],
            withDist: true
        );

        $this->assertNotEmpty($results);
        $this->assertTrue(
            array_reduce($results, fn($carry, $r) => $carry || $r['member'] === 'San Francisco', false)
        );
    }

    public function testGeoSearchFromLonLatByRadius(): void
    {
        $key = $this->testKey . ':geosearch:lonlat';
        $this->geospatial->geoAdd($key, [
            ['lat' => 37.7749, 'lon' => -122.4194, 'member' => 'San Francisco'],
            ['lat' => 37.8044, 'lon' => -122.2711, 'member' => 'Oakland'],
        ]);

        $results = $this->geospatial->geoSearch(
            $key,
            fromLonLat: [-122.4194, 37.7749],
            byRadius: [50.0, 'km'],
            withDist: true,
            withCoord: true
        );

        $this->assertNotEmpty($results);
    }

    public function testGeoSearchByBox(): void
    {
        $key = $this->testKey . ':geosearch:box';
        $this->geospatial->geoAdd($key, [
            ['lat' => 37.7749, 'lon' => -122.4194, 'member' => 'San Francisco'],
            ['lat' => 37.8044, 'lon' => -122.2711, 'member' => 'Oakland'],
        ]);

        $results = $this->geospatial->geoSearch(
            $key,
            fromMember: 'San Francisco',
            byBox: [100000.0, 100000.0, 'm'],
            withCoord: true
        );

        $this->assertNotEmpty($results);
    }

    public function testStats(): void
    {
        $stats = $this->geospatial->stats();

        $this->assertArrayHasKey('total_keys', $stats);
        $this->assertArrayHasKey('total_locations', $stats);
        $this->assertArrayHasKey('geoadd_count', $stats);
        $this->assertGreaterThanOrEqual(0, $stats['total_keys']);
    }
}

