<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Exception\SynapException;
use Synap\SDK\Module\HyperLogLogManager;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

/**
 * Server-to-Server (S2S) integration tests for HyperLogLogManager.
 * These tests require a running Synap server.
 * Set SYNAP_URL environment variable to point to the server (default: http://localhost:15500).
 */
class HyperLogLogManagerS2STest extends TestCase
{
    private SynapClient $client;
    private HyperLogLogManager $hyperloglog;

    protected function setUp(): void
    {
        parent::setUp();

        $url = getenv('SYNAP_URL') ?: ($_ENV['SYNAP_URL'] ?? 'http://localhost:15500');
        $config = new SynapConfig($url);
        $this->client = new SynapClient($config);
        $this->hyperloglog = $this->client->hyperloglog();
    }

    protected function tearDown(): void
    {
        // Properties are automatically cleared by PHPUnit
        parent::tearDown();
    }

    public function testPfAddPfCount(): void
    {
        $key = 'test:hll:' . getmypid();

        $added = $this->hyperloglog->pfAdd($key, ['user:1', 'user:2', 'user:3']);
        $this->assertGreaterThanOrEqual(0, $added);
        $this->assertLessThanOrEqual(3, $added);

        $count = $this->hyperloglog->pfCount($key);
        // Approximate, may be slightly off
        $this->assertGreaterThanOrEqual(2, $count);
        $this->assertLessThanOrEqual(4, $count);
    }

    public function testPfMerge(): void
    {
        $timestamp = getmypid();
        $key1 = "test:hll:merge1:{$timestamp}";
        $key2 = "test:hll:merge2:{$timestamp}";
        $dest = "test:hll:merge_dest:{$timestamp}";

        $this->hyperloglog->pfAdd($key1, ['user:1', 'user:2', 'user:3']);
        $this->hyperloglog->pfAdd($key2, ['user:4', 'user:5', 'user:6']);

        $count = $this->hyperloglog->pfMerge($dest, [$key1, $key2]);
        // Approximate
        $this->assertGreaterThanOrEqual(5, $count);
        $this->assertLessThanOrEqual(7, $count);
    }

    public function testStats(): void
    {
        $key = 'test:hll:stats:' . getmypid();

        $this->hyperloglog->pfAdd($key, ['user:1', 'user:2']);
        $this->hyperloglog->pfCount($key);

        $stats = $this->hyperloglog->stats();
        $this->assertGreaterThanOrEqual(1, $stats['pfadd_count']);
        $this->assertGreaterThanOrEqual(1, $stats['pfcount_count']);
    }
}

