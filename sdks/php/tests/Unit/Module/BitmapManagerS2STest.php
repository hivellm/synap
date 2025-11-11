<?php

declare(strict_types=1);

namespace Synap\SDK\Tests\Unit\Module;

use PHPUnit\Framework\TestCase;
use Synap\SDK\Exception\SynapException;
use Synap\SDK\Module\BitmapManager;
use Synap\SDK\SynapClient;
use Synap\SDK\SynapConfig;

/**
 * Server-to-Server (S2S) integration tests for BitmapManager.
 * These tests require a running Synap server.
 * Set SYNAP_URL environment variable to point to the server (default: http://localhost:15500).
 */
class BitmapManagerS2STest extends TestCase
{
    private SynapClient $client;
    private BitmapManager $bitmap;

    protected function setUp(): void
    {
        parent::setUp();

        $url = getenv('SYNAP_URL') ?: ($_ENV['SYNAP_URL'] ?? 'http://localhost:15500');
        $config = new SynapConfig($url);
        $this->client = new SynapClient($config);
        $this->bitmap = $this->client->bitmap();
    }

    protected function tearDown(): void
    {
        // Properties are automatically cleared by PHPUnit
        parent::tearDown();
    }

    public function testSetBitGetBit(): void
    {
        $key = 'test:bitmap:' . getmypid();

        // Set bit 5 to 1
        $oldValue = $this->bitmap->setBit($key, 5, 1);
        $this->assertEquals(0, $oldValue);

        // Get bit 5
        $value = $this->bitmap->getBit($key, 5);
        $this->assertEquals(1, $value);

        // Set bit 5 back to 0
        $oldValue2 = $this->bitmap->setBit($key, 5, 0);
        $this->assertEquals(1, $oldValue2);

        // Get bit 5 again
        $value2 = $this->bitmap->getBit($key, 5);
        $this->assertEquals(0, $value2);
    }

    public function testBitCount(): void
    {
        $key = 'test:bitmap:count:' . getmypid();

        // Set multiple bits
        $this->bitmap->setBit($key, 0, 1);
        $this->bitmap->setBit($key, 2, 1);
        $this->bitmap->setBit($key, 4, 1);
        $this->bitmap->setBit($key, 6, 1);

        // Count all bits
        $count = $this->bitmap->bitCount($key);
        $this->assertEquals(4, $count);
    }

    public function testBitPos(): void
    {
        $key = 'test:bitmap:pos:' . getmypid();

        // Set bit at position 7
        $this->bitmap->setBit($key, 7, 1);

        // Find first set bit
        $pos = $this->bitmap->bitPos($key, 1);
        $this->assertEquals(7, $pos);
    }

    public function testBitOpAnd(): void
    {
        $timestamp = getmypid();
        $key1 = "test:bitmap:and1:{$timestamp}";
        $key2 = "test:bitmap:and2:{$timestamp}";
        $dest = "test:bitmap:and_result:{$timestamp}";

        // Set bits in bitmap1 (bits 0, 1, 2)
        $this->bitmap->setBit($key1, 0, 1);
        $this->bitmap->setBit($key1, 1, 1);
        $this->bitmap->setBit($key1, 2, 1);

        // Set bits in bitmap2 (bits 1, 2, 3)
        $this->bitmap->setBit($key2, 1, 1);
        $this->bitmap->setBit($key2, 2, 1);
        $this->bitmap->setBit($key2, 3, 1);

        // AND operation
        $length = $this->bitmap->bitOp('AND', $dest, [$key1, $key2]);
        $this->assertGreaterThan(0, $length);

        // Check result: should have bits 1 and 2 set
        $this->assertEquals(0, $this->bitmap->getBit($dest, 0));
        $this->assertEquals(1, $this->bitmap->getBit($dest, 1));
        $this->assertEquals(1, $this->bitmap->getBit($dest, 2));
        $this->assertEquals(0, $this->bitmap->getBit($dest, 3));
    }

    public function testBitFieldGetSet(): void
    {
        $key = 'test:bitmap:bitfield:' . getmypid();

        // SET operation: Set 8-bit unsigned value 42 at offset 0
        $operations = [
            [
                'operation' => 'SET',
                'offset' => 0,
                'width' => 8,
                'signed' => false,
                'value' => 42,
            ],
        ];

        $results = $this->bitmap->bitField($key, $operations);
        $this->assertCount(1, $results);
        $this->assertEquals(0, $results[0]); // Old value was 0

        // GET operation: Read back the value
        $operations = [
            [
                'operation' => 'GET',
                'offset' => 0,
                'width' => 8,
                'signed' => false,
            ],
        ];

        $results = $this->bitmap->bitField($key, $operations);
        $this->assertCount(1, $results);
        $this->assertEquals(42, $results[0]);
    }

    public function testBitFieldIncrByWrap(): void
    {
        $key = 'test:bitmap:bitfield_wrap:' . getmypid();

        // Set initial value
        $operations = [
            [
                'operation' => 'SET',
                'offset' => 0,
                'width' => 8,
                'signed' => false,
                'value' => 250,
            ],
        ];
        $this->bitmap->bitField($key, $operations);

        // INCRBY with wrap: 250 + 10 = 260 wraps to 4
        $operations = [
            [
                'operation' => 'INCRBY',
                'offset' => 0,
                'width' => 8,
                'signed' => false,
                'increment' => 10,
                'overflow' => 'WRAP',
            ],
        ];

        $results = $this->bitmap->bitField($key, $operations);
        $this->assertCount(1, $results);
        $this->assertEquals(4, $results[0]); // 250 + 10 = 260 wraps to 4 (260 - 256)
    }

    public function testBitFieldIncrBySat(): void
    {
        $key = 'test:bitmap:bitfield_sat:' . getmypid();

        // Set 4-bit unsigned value to 14
        $operations = [
            [
                'operation' => 'SET',
                'offset' => 0,
                'width' => 4,
                'signed' => false,
                'value' => 14,
            ],
        ];
        $this->bitmap->bitField($key, $operations);

        // INCRBY with saturate: 14 + 1 = 15 (max), then stays at 15
        $operations = [
            [
                'operation' => 'INCRBY',
                'offset' => 0,
                'width' => 4,
                'signed' => false,
                'increment' => 1,
                'overflow' => 'SAT',
            ],
        ];

        $results1 = $this->bitmap->bitField($key, $operations);
        $this->assertEquals(15, $results1[0]);

        // Try to increment again (should saturate at 15)
        $results2 = $this->bitmap->bitField($key, $operations);
        $this->assertEquals(15, $results2[0]);
    }

    public function testBitFieldMultipleOperations(): void
    {
        $key = 'test:bitmap:bitfield_multi:' . getmypid();

        // Execute multiple operations in sequence
        $operations = [
            [
                'operation' => 'SET',
                'offset' => 0,
                'width' => 8,
                'signed' => false,
                'value' => 100,
            ],
            [
                'operation' => 'SET',
                'offset' => 8,
                'width' => 8,
                'signed' => false,
                'value' => 200,
            ],
            [
                'operation' => 'GET',
                'offset' => 0,
                'width' => 8,
                'signed' => false,
            ],
            [
                'operation' => 'GET',
                'offset' => 8,
                'width' => 8,
                'signed' => false,
            ],
            [
                'operation' => 'INCRBY',
                'offset' => 0,
                'width' => 8,
                'signed' => false,
                'increment' => 50,
                'overflow' => 'WRAP',
            ],
        ];

        $results = $this->bitmap->bitField($key, $operations);
        $this->assertCount(5, $results);
        $this->assertEquals(0, $results[0]); // Old value at offset 0
        $this->assertEquals(0, $results[1]); // Old value at offset 8
        $this->assertEquals(100, $results[2]); // Read back offset 0
        $this->assertEquals(200, $results[3]); // Read back offset 8
        $this->assertEquals(150, $results[4]); // Incremented offset 0
    }

    public function testBitFieldSignedValues(): void
    {
        $key = 'test:bitmap:bitfield_signed:' . getmypid();

        // Set signed 8-bit negative value
        $operations = [
            [
                'operation' => 'SET',
                'offset' => 0,
                'width' => 8,
                'signed' => true,
                'value' => -10,
            ],
        ];
        $this->bitmap->bitField($key, $operations);

        // Read back as signed
        $operations = [
            [
                'operation' => 'GET',
                'offset' => 0,
                'width' => 8,
                'signed' => true,
            ],
        ];

        $results = $this->bitmap->bitField($key, $operations);
        $this->assertCount(1, $results);
        $this->assertEquals(-10, $results[0]);
    }

    public function testStats(): void
    {
        $key = 'test:bitmap:stats:' . getmypid();

        // Perform some operations
        $this->bitmap->setBit($key, 0, 1);
        $this->bitmap->getBit($key, 0);
        $this->bitmap->bitCount($key);

        $stats = $this->bitmap->stats();
        $this->assertGreaterThanOrEqual(1, $stats['setbit_count']);
        $this->assertGreaterThanOrEqual(1, $stats['getbit_count']);
        $this->assertGreaterThanOrEqual(1, $stats['bitcount_count']);
    }
}

